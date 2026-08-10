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
//! 単純な操作（`data_insert` など）は最初の宣言で判明するため、やり直しの 1 周目は
//! I/O を伴わない。`Storage::write` のシグネチャは変わらず、サービス層はロックの存在を
//! 知らないままでいられる。

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

/// ロック不足でやり直すことを示す内部専用のエラー。
///
/// [`Storage::write`] がクロージャの結果を見る前にロック充足を確認するため、この値が
/// 呼び出し元へ漏れることはない。
fn restart_sentinel() -> AppError {
    AppError::InternalError("lock declaration requires a restart".to_string())
}

/// TiKV が「やり直せば通る」種類の失敗を返したか。
///
/// ロック保持者がコミットすると待機側は `WriteConflict { reason: PessimisticRetry }` を
/// 受け取る。tikv-client はこれを内部でリトライせず呼び出し側へ委ねるため、ここで判定して
/// 自分でやり直す。こうすることで「書き込みは待たされても失敗しない」という
/// LMDB の性質が呼び出し側から見て保たれる。
fn is_retryable(err: &tikv_client::Error) -> bool {
    let s = format!("{err:?}");
    s.contains("PessimisticRetry") || s.contains("WriteConflict") || s.contains("Deadlock")
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
    /// ロックキーを**渡された順序どおりに**取得する。
    ///
    /// 呼び出し側は常に同じ順序（データベース単位 → テーブル単位）で並べること。
    /// 全操作がこの順序を守る限り、待ちグラフに循環ができずデッドロックしない。
    async fn acquire(
        client: &TransactionClient,
        keys: &BTreeSet<Vec<u8>>,
    ) -> Result<Self, tikv_client::Error> {
        let mut txn = client.begin_pessimistic().await?;
        // まとめて渡す。`lock_keys` はリージョンごとに 1 リクエストへ束ねるので、
        // 1 キーずつ呼ぶとロック数だけ往復が増える。`BTreeSet` の反復順が
        // そのままデッドロック回避に必要な固定順序になる。
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
pub struct TikvRead<'a> {
    pub(crate) txn: tokio::sync::Mutex<Transaction>,
    pub(crate) _db: &'a TikvDb,
}

/// 書き込みトランザクション。
pub struct TikvWrite<'a> {
    pub(crate) txn: tokio::sync::Mutex<Transaction>,
    pub(crate) _db: &'a TikvDb,
    /// この試行で保持しているロック。
    held: BTreeSet<Vec<u8>>,
    /// 操作が宣言したが、まだ保持していないロック。
    /// 空でなければ、この試行は巻き戻してやり直す。
    missing: BTreeSet<Vec<u8>>,
}

impl TikvWrite<'_> {
    /// この操作が触る範囲を宣言する。**実際にデータへ触れる前**に呼ぶこと。
    ///
    /// まだ取得していないロックだった場合は [`RESTART_SENTINEL`] を返す。呼び出し側が
    /// `?` で伝播させれば、その試行は破棄され、宣言されたロックを揃えた状態で
    /// クロージャが最初から実行し直される。
    ///
    /// 宣言忘れをコンパイラに検出させるため、確認を戻り値に持たせている
    /// （フラグを別途調べる方式では、確認を書き忘れてもコンパイルが通ってしまう）。
    pub(crate) fn require_lock(&mut self, scope: LockScope, id: &[u8]) -> Result<(), AppError> {
        let key = keys::lock(scope, id);
        if self.held.contains(&key) {
            return Ok(());
        }
        self.missing.insert(key);
        Err(restart_sentinel())
    }

    /// 複数スコープをまとめて宣言する。
    ///
    /// 1 つずつ宣言してやり直すと 1 回の試行で 1 つしかロックを集められないので、
    /// 複数必要な操作はここで全部宣言してから戻る。
    /// 順序は呼び出し側の責任（データベース単位 → テーブル単位）。
    pub(crate) fn require_locks<'k>(
        &mut self,
        scopes: impl IntoIterator<Item = (LockScope, &'k [u8])>,
    ) -> Result<(), AppError> {
        let mut missing_any = false;
        for (scope, id) in scopes {
            if self.require_lock(scope, id).is_err() {
                missing_any = true;
            }
        }
        if missing_any {
            return Err(restart_sentinel());
        }
        Ok(())
    }

    fn needs_more_locks(&self) -> bool {
        !self.missing.is_empty()
    }
}

impl Storage for TikvDb {
    type Read<'a> = TikvRead<'a>;
    type Write<'a> = TikvWrite<'a>;

    async fn read<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a Self::Read<'b>) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static,
    {
        // 読み取りはロックを取らない。スナップショットから読むだけなので
        // 書き込みをブロックせず、書き込みにもブロックされない（LMDB と同じ）。
        let txn = self.client.begin_optimistic().await.map_err(to_app_error)?;
        let r = TikvRead {
            txn: tokio::sync::Mutex::new(txn),
            _db: self,
        };
        let out = f(&r).await;
        let mut txn = r.txn.into_inner();
        let _ = txn.rollback().await;
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
                _db: self,
                held: locks.clone(),
                missing: BTreeSet::new(),
            };

            // やり直しに備えて複製を渡す。`f` 自体は次の試行のために残す。
            let result = f.clone()(&mut w).await;
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

        Err(last_error.unwrap_or_else(|| {
            AppError::Conflict("write retries exhausted due to lock contention".to_string())
        }))
    }
}
