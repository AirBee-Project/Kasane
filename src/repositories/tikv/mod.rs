//! TiKV バックエンド。
//!
//! ファイル構成は LMDB 実装（`super::lmdb`）と対になっている。
//!
//! | ファイル | 役割 |
//! |---|---|
//! | `mod.rs` | ストレージ本体と、トランザクション境界（[`Storage`] 実装） |
//! | `init.rs` | このバックエンド固有の初期化設定 |
//! | `keys.rs` | キーのバイト表現 |
//! | `kv.rs` | このバックエンド固有の低レベルアクセス（LMDB 側は `shard.rs`） |
//! | `catalog.rs` | データベース・テーブルのカタログ操作 |
//! | `data.rs` | FlexTree のデータ操作 |
//! | `users.rs` | ユーザーと権限 |
//! | `query_source.rs` | クエリ実行器への入力源 |
//! | `repository.rs` | 抽象 trait への適合 |
//!
//! # トランザクションの組み立て方
//!
//! TiKV の悲観ロックは取得時に取り直した `for_update_ts` で取られるのに対し、
//! `txn.get()` はトランザクション開始時の `start_ts` スナップショットを読む。
//! そのため「1 つのトランザクション内でロックしてから読む」と、ロック取得前に
//! コミットされた他者の変更を見落として lost update になる（実測済み。
//! `docs/tikv-migration-phase0.md` を参照）。
//!
//! そこで LMDB の `env.write_txn()` と同じ順序――**ライタミューテックス取得 →
//! スナップショット確定**――を 2 つのトランザクションに分けて再現する。
//!
//! 1. ロック専用トランザクションで必要なロックキーを取得する
//! 2. **その後に**作業トランザクションを開始する（start_ts が前任者のコミットより後になる）
//! 3. 作業をコミットする
//! 4. ロック専用トランザクションを rollback して解放する
//!
//! ロック側は常に rollback で終わるので、ロックキーには MVCC のバージョンが作られない。
//!
//! # 必要なロックをどう知るか
//!
//! `database_remove` のように「ロックを取って初めて対象が判る」操作があるため、
//! 呼び出し前にロック集合を列挙することはできない。書き込みクロージャは競合時の
//! やり直しに備えて元々**複数回呼ばれてよい**設計なので、これを利用する：
//!
//! - 各操作は触る範囲を [`TikvWrite::require_lock`] で宣言する
//! - まだ保持していないロックが宣言されたら、その場で巻き戻して
//!   **集めたロックを揃えた状態でやり直す**
//!
//! `Storage::write` のシグネチャは変わらず、サービス層はロックの存在を知らないままでいられる。
//!
//! # デッドロックしない理由
//!
//! 保持中／不足中のロックはどちらも [`BTreeSet<Vec<u8>>`] で持つので、取得は常に
//! **ロックキーのバイト昇順**になる。キーは `0x7F ‖ scope ‖ id`（`keys.rs`）で、
//! [`LockScope`] の判別値が Database < Table < User と並ぶよう振られているため、
//! この昇順がそのまま「データベース単位 → テーブル単位 → ユーザー単位」の階層順になる。
//! 全操作が同じ全順序でロックを取るので、待ちグラフに循環ができない。
//!
//! 順序は集合の型が決めるので、呼び出し側が並べ替える必要はない。ただし
//! [`tikv_client::Transaction::lock_keys`] はリージョンごとにリクエストを束ねるため、
//! 1 回の呼び出しの**内部**での取得順までは保証されない。そこで最後の砦として、
//! TiKV 側が検出したデッドロックは [`is_retryable`] が拾ってやり直す。

mod catalog;
mod data;
mod init;
mod keys;
mod kv;
mod query_source;
mod repository;
mod users;

pub use init::TikvConfig;
pub use query_source::TikvTableSource;

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::sync::Arc;

use tikv_client::{Transaction, TransactionClient};

use crate::error::AppError;
use crate::repositories::Storage;
use keys::LockScope;

/// 競合・ロック待ちでやり直す上限。
const MAX_ATTEMPTS: usize = 20;

fn to_app_error(err: tikv_client::Error) -> AppError {
    AppError::StorageError(err.to_string())
}

/// ロックが足りないので、この試行を捨ててやり直すことを示すマーカー。
///
/// [`AppError`] とは別の型にしてあるのが要点で、[`TikvWrite::require_lock`] の戻り値を
/// 見ればそれが「アプリケーションの失敗」ではなく「制御用の巻き戻し」だと型で判る。
/// 操作側は `?` で伝播させるだけでよく、その際 [`From`] でアプリケーションエラーへ落ちる。
///
/// [`Storage::write`] はクロージャの結果を見る**前に**ロック充足を確認するので、
/// 変換後の値が呼び出し元へ届くことはない。届いたとすればそれは実装のバグであり、
/// メッセージからそう判るようにしてある。
#[derive(Debug, Clone, Copy)]
pub(crate) struct NeedsRestart;

impl From<NeedsRestart> for AppError {
    fn from(_: NeedsRestart) -> Self {
        AppError::InternalError(
            "lock declaration escaped the write retry loop (this is a bug in the tikv backend)"
                .to_string(),
        )
    }
}

/// TiKV が「やり直せば通る」種類の失敗を返したか。
///
/// ロック保持者がコミットすると待機側は書き込み競合として弾かれる。tikv-client は
/// これを内部でリトライせず呼び出し側へ委ねるため、ここで判定して自分でやり直す。
/// こうすることで「書き込みは待たされても失敗しない」という LMDB の性質が
/// 呼び出し側から見て保たれる。
///
/// 判定は `KeyError` のフィールドを直接見る。Debug 文字列の部分一致だと、
/// 利用者が付けた名前がたまたま一致して誤ってやり直したり、tikv-client 側の
/// 文言変更で黙って「待てば通る失敗」が 500 に化けたりする。
fn is_retryable(err: &tikv_client::Error) -> bool {
    use tikv_client::Error;

    match err {
        // `conflict` は書き込み競合（悲観トランザクションでの `PessimisticRetry` を含む）、
        // `deadlock` は TiKV のデッドロック検出、`retryable` は
        // 「クライアントはトランザクションをやり直してよい」という明示の指示。
        Error::KeyError(key_error) => {
            key_error.conflict.is_some()
                || key_error.deadlock.is_some()
                || !key_error.retryable.is_empty()
        }
        Error::MultipleKeyErrors(errors) | Error::ExtractedErrors(errors) => {
            errors.iter().any(is_retryable)
        }
        Error::PessimisticLockError { inner, .. } => is_retryable(inner),
        // `UndeterminedError` はコミットの成否が不明なので、やり直すと二重適用に
        // なりうる。ここでは拾わず、呼び出し元へそのまま伝える。
        _ => false,
    }
}

/// TiKV バックエンドのハンドル。複製してもクラスタへの接続は共有される。
///
/// 構築は `init.rs`（[`TikvDb::connect`]）が担う。
#[derive(Clone)]
pub struct TikvDb {
    pub(super) client: Arc<TransactionClient>,
}

/// 保持中のロック。解放は必ず rollback で行う。
struct LockGuard {
    txn: Transaction,
}

impl LockGuard {
    /// ロックキーをまとめて取得する。
    ///
    /// `keys` が [`BTreeSet`] なのは偶然ではない。反復順（＝バイト昇順）が
    /// そのままデッドロック回避に必要な全順序になる（モジュール冒頭を参照）。
    /// まとめて渡すのは、`lock_keys` がリージョンごとに 1 リクエストへ束ねるため。
    /// 1 キーずつ呼ぶとロック数だけ往復が増える。
    async fn acquire(
        client: &TransactionClient,
        keys: &BTreeSet<Vec<u8>>,
    ) -> Result<Self, tikv_client::Error> {
        let mut txn = client.begin_pessimistic().await?;
        if let Err(e) = txn.lock_keys(keys.iter().cloned()).await {
            let _ = txn.rollback().await;
            return Err(e);
        }
        Ok(Self { txn })
    }

    async fn release(mut self) {
        let _ = self.txn.rollback().await;
    }
}

/// 読み取りトランザクション。
///
/// `R` は読み取り元のスナップショット。通常の読み取りは [`Transaction`]、
/// クエリ実行器の入力源は開始タイムスタンプを固定した [`tikv_client::Snapshot`] を使う
/// （`query_source.rs` を参照）。
pub struct TikvRead<'a, R = Transaction> {
    pub(crate) txn: tokio::sync::Mutex<R>,
    /// ストレージのハンドルより長生きしないことを型で示すだけのマーカー。
    pub(crate) _db: PhantomData<&'a TikvDb>,
}

impl<R> TikvRead<'_, R> {
    pub(crate) fn new(reader: R) -> Self {
        Self {
            txn: tokio::sync::Mutex::new(reader),
            _db: PhantomData,
        }
    }

    pub(crate) fn into_inner(self) -> R {
        self.txn.into_inner()
    }
}

/// 書き込みトランザクション。
pub struct TikvWrite<'a> {
    pub(crate) txn: tokio::sync::Mutex<Transaction>,
    pub(crate) _db: PhantomData<&'a TikvDb>,
    /// この試行で保持しているロック。
    held: BTreeSet<Vec<u8>>,
    /// 操作が宣言したが、まだ保持していないロック。
    /// 空でなければ、この試行は巻き戻してやり直す。
    missing: BTreeSet<Vec<u8>>,
}

impl TikvWrite<'_> {
    /// この操作が触る範囲を宣言する。**実際にデータへ触れる前**に呼ぶこと。
    ///
    /// まだ取得していないロックだった場合は [`NeedsRestart`] を返す。呼び出し側が
    /// `?` で伝播させれば、その試行は破棄され、宣言されたロックを揃えた状態で
    /// クロージャが最初から実行し直される。
    ///
    /// 宣言忘れをコンパイラに検出させるため、確認を戻り値に持たせている
    /// （フラグを別途調べる方式では、確認を書き忘れてもコンパイルが通ってしまう）。
    pub(crate) fn require_lock(&mut self, scope: LockScope, id: &[u8]) -> Result<(), NeedsRestart> {
        let key = keys::lock(scope, id);
        if self.held.contains(&key) {
            return Ok(());
        }
        self.missing.insert(key);
        Err(NeedsRestart)
    }

    /// 複数スコープをまとめて宣言する。
    ///
    /// 1 つずつ宣言してやり直すと 1 回の試行で 1 つしかロックを集められないので、
    /// 複数必要な操作はここで全部宣言してから戻る。取得順は集合が決めるので、
    /// 渡す順序は問わない（モジュール冒頭のデッドロックの節を参照）。
    pub(crate) fn require_locks<'k>(
        &mut self,
        scopes: impl IntoIterator<Item = (LockScope, &'k [u8])>,
    ) -> Result<(), NeedsRestart> {
        let mut missing_any = false;
        for (scope, id) in scopes {
            if self.require_lock(scope, id).is_err() {
                missing_any = true;
            }
        }
        if missing_any {
            return Err(NeedsRestart);
        }
        Ok(())
    }

    fn needs_more_locks(&self) -> bool {
        !self.missing.is_empty()
    }
}

impl Storage for TikvDb {
    type Read<'a> = TikvRead<'a, Transaction>;
    type Write<'a> = TikvWrite<'a>;
    /// 断面は開始タイムスタンプそのもの。各読み取りはこの時刻の
    /// [`tikv_client::Snapshot`] を開く（`query_source.rs`）。
    type QuerySnapshot = tikv_client::Timestamp;

    async fn query_snapshot(&self) -> Result<Self::QuerySnapshot, AppError> {
        self.client.current_timestamp().await.map_err(to_app_error)
    }

    async fn read<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a Self::Read<'b>) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static,
    {
        // 読み取りはロックを取らない。スナップショットから読むだけなので
        // 書き込みをブロックせず、書き込みにもブロックされない（LMDB と同じ）。
        let txn = self.client.begin_optimistic().await.map_err(to_app_error)?;
        let r = TikvRead::new(txn);
        let out = f(&r).await;
        let _ = r.into_inner().rollback().await;
        out
    }

    async fn write<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a mut Self::Write<'b>) -> Result<T, AppError>
            + Clone
            + Send
            + 'static,
        T: Send + 'static,
    {
        let mut locks: BTreeSet<Vec<u8>> = BTreeSet::new();
        let mut last_error: Option<AppError> = None;

        for _ in 0..MAX_ATTEMPTS {
            // 1. ロックを取得する（初回は空集合＝ロックなしで走らせ、必要な範囲を宣言させる）。
            let guard = match LockGuard::acquire(&self.client, &locks).await {
                Ok(guard) => guard,
                Err(e) if is_retryable(&e) => {
                    last_error = Some(to_app_error(e));
                    continue;
                }
                Err(e) => return Err(to_app_error(e)),
            };

            // 2. ロックを保持した状態で作業トランザクションを開始する。
            //    ここで start_ts が確定するので、前任者のコミットが必ず見える。
            let txn = match self.client.begin_pessimistic().await {
                Ok(txn) => txn,
                Err(e) => {
                    guard.release().await;
                    return Err(to_app_error(e));
                }
            };
            let mut w = TikvWrite {
                txn: tokio::sync::Mutex::new(txn),
                _db: PhantomData,
                held: locks.clone(),
                missing: BTreeSet::new(),
            };

            // やり直しに備えて複製を渡す。`f` 自体は次の試行のために残す。
            let result = f.clone()(&mut w).await;
            // ロック充足の確認は**結果より先**に行う。こうしておけば、クロージャが
            // `NeedsRestart` 由来のエラーを握り潰して `Ok` を返しても、その試行は
            // 確実に捨てられる（＝宣言し忘れた範囲へ書いたまま commit されない）。
            let need_more = w.needs_more_locks();
            let newly_required = std::mem::take(&mut w.missing);
            let mut txn = w.txn.into_inner();

            // 3. ロックが足りなければ、この試行は捨ててロックを揃えてやり直す。
            if need_more {
                let _ = txn.rollback().await;
                guard.release().await;
                locks.extend(newly_required);
                continue;
            }

            let outcome = match result {
                Ok(value) => match txn.commit().await {
                    Ok(_) => Ok(value),
                    Err(e) => Err(e),
                },
                Err(app_err) => {
                    // クロージャが失敗したらコミットしない。ロックは必ず解放する。
                    let _ = txn.rollback().await;
                    guard.release().await;
                    return Err(app_err);
                }
            };

            guard.release().await;

            match outcome {
                Ok(value) => return Ok(value),
                Err(e) if is_retryable(&e) => {
                    last_error = Some(to_app_error(e));
                    continue;
                }
                Err(e) => return Err(to_app_error(e)),
            }
        }

        // ここへ来る理由は 2 つある。競合し続けた（`last_error` あり）か、
        // ロック宣言が収束しなかった（実装のバグ）か。区別できるようにしておく。
        Err(last_error.unwrap_or_else(|| {
            AppError::Conflict(format!(
                "write did not settle within {MAX_ATTEMPTS} attempts ({} lock(s) held at the end)",
                locks.len()
            ))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `LockScope` の判別値が「データベース → テーブル → ユーザー」の順に並ぶこと。
    ///
    /// ロックキーのバイト昇順がそのまま取得順になるので、この並びが崩れると
    /// デッドロック回避の前提が壊れる。
    #[test]
    fn lock_scopes_sort_into_the_hierarchy_order() {
        let db = keys::lock(LockScope::Database, b"z-database");
        let table = keys::lock(LockScope::Table, b"a-table");
        let user = keys::lock(LockScope::User, b"a-user");

        // id の中身に関わらず、スコープの順序が先に効く。
        assert!(db < table);
        assert!(table < user);

        let ordered: Vec<_> = BTreeSet::from([user.clone(), table.clone(), db.clone()])
            .into_iter()
            .collect();
        assert_eq!(ordered, vec![db, table, user]);
    }

    #[test]
    fn restart_marker_is_labelled_as_a_bug_when_converted() {
        let err: AppError = NeedsRestart.into();
        let AppError::InternalError(message) = err else {
            panic!("NeedsRestart は InternalError へ落ちるべき");
        };
        assert!(message.contains("bug"), "実装バグと判る文言であること");
    }
}
