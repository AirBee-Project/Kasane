//! TiKV バックエンド。
//!
//! 排他の付け方は経路で違う。データ経路はシャードのキー単位（`kv::lock_shards`）、
//! カタログ経路は明示ロック（`lock_options` と `TikvWrite::require_lock`）。

#![allow(clippy::result_large_err)]

mod catalog;
mod gc;
mod init;
mod keys;
mod kv;
mod query_source;
mod repository;
mod tree;
mod users;

pub use gc::GcConfig;
pub use init::TikvConfig;
pub use query_source::TikvTableSource;

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::sync::Arc;

use rand_core::RngCore;
use tikv_client::{CheckLevel, Snapshot, Transaction, TransactionClient, TransactionOptions};

use crate::error::AppError;
use crate::repositories::Storage;
use keys::LockScope;
use kv::LazyTxn;

/// 競合・ロック待ちでやり直す上限。
const MAX_ATTEMPTS: usize = 20;

/// 待たずに `continue` すると、混み合っている相手へ再挑戦を詰めて投げることになる。
const RETRY_BACKOFF_BASE: std::time::Duration = std::time::Duration::from_millis(5);

/// 上限が小さいと、重い試行（クロージャ全体のやり直し）を隙間なく投げ続けることになる。
/// ロックの寿命 20 秒を超えたら何度やっても通らないので、その手前まで伸ばす。
const RETRY_BACKOFF_MAX: std::time::Duration = std::time::Duration::from_millis(2000);

/// 倍々に伸びるだけだと、ぶつかったインスタンスが揃った間隔でやり直し続ける。TiKV の悲観
/// ロックは待たず即エラーを返すので、競合の解消はこちらのやり直しに委ねられている。
const RETRY_BACKOFF_JITTER_PERCENT: u64 = 20;

/// 競合回数と直近の失敗理由を組にするのは、片方だけ更新する食い違いを型で防ぐため。
#[derive(Default)]
struct Retry {
    conflicts: u32,
    last_error: Option<AppError>,
}

impl Retry {
    async fn back_off(&mut self, reason: AppError) {
        self.last_error = Some(reason);
        tokio::time::sleep(retry_backoff(self.conflicts)).await;
        self.conflicts += 1;
    }

    /// 待たずにやり直す場合。競合ではないので回数には数えない。
    fn note(&mut self, reason: AppError) {
        self.last_error = Some(reason);
    }
}

/// `min(16)` は桁溢れ避け。ここを [`RETRY_BACKOFF_MAX`] より手前にすると上限が効かなくなる。
fn retry_backoff(attempt: u32) -> std::time::Duration {
    let base = RETRY_BACKOFF_BASE
        .saturating_mul(1u32 << attempt.min(16))
        .min(RETRY_BACKOFF_MAX);
    apply_jitter(base)
}

fn apply_jitter(base: std::time::Duration) -> std::time::Duration {
    let millis = base.as_millis() as u64;
    let span = millis * RETRY_BACKOFF_JITTER_PERCENT / 100;
    if span == 0 {
        // 揺らぎが 1ms 未満になる短さでは、丸めると意味を持たない。
        return base;
    }
    let mut bytes = [0u8; 8];
    rand_core::OsRng.fill_bytes(&mut bytes);
    let offset = u64::from_le_bytes(bytes) % (span * 2 + 1);
    std::time::Duration::from_millis(millis - span + offset)
}

impl From<tikv_client::Error> for AppError {
    fn from(err: tikv_client::Error) -> Self {
        Self::StorageError(err.to_string())
    }
}

// 放置された [`Transaction`] の drop で panic しないよう、必ず下の 3 つを通して開くこと。

fn read_options() -> TransactionOptions {
    TransactionOptions::new_optimistic().drop_check(CheckLevel::Warn)
}

/// カタログ経路の明示ロック専用。**必ず悲観にすること。**
///
/// 範囲スキャンから変更を導く操作はキー単位の競合検査では守れない（`database_remove` と
/// `table_create` は書くキーが重ならないのに、両方成功するとテーブルが取り残される）。
/// 楽観だと `lock_keys` が RPC を出さず、検査は必ず rollback するこのトランザクションの
/// コミット時に回るので、**排他が黙って消える**。
fn lock_options() -> TransactionOptions {
    TransactionOptions::new_pessimistic().drop_check(CheckLevel::Warn)
}

/// 作業（書き込み）トランザクションの設定。**悲観**。
///
/// 楽観だと競合の検出が prewrite まで遅れ、1 回の競合で捨てる仕事が降下から直列化まで全部に
/// なる。書き込みが集中する使い方では 6 割超が 2 回以上やり直した。低競合が前提なら楽観へ
/// 寄せてよいが、そのとき [`lock_options`] は悲観のまま残すこと。
///
/// `one_pc` を TiKV が断ると [`tikv_client::Error::OnePcFailure`] になる。
/// **フォールバックは無い**ので呼び出し側が 2PC でやり直す。
fn write_options(one_pc: bool) -> TransactionOptions {
    let options = TransactionOptions::new_pessimistic().drop_check(CheckLevel::Warn);
    if one_pc {
        options.try_one_pc()
    } else {
        options
    }
}

/// Future が途中で捨てられてもロックだけは返しにいく。握ったまま消えると、TTL が切れるまで
/// 同じシャードへの書き込みが止まる。
fn rollback_in_background(mut txn: Transaction) {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(async move {
                let _ = txn.rollback().await;
            });
        }
        Err(_) => drop(txn),
    }
}

/// この試行を捨ててやり直すことを示すマーカー。
///
/// [`AppError`] と別の型にしてあるので「制御用の巻き戻し」だと型で判る。[`Storage::write`] は
/// 結果を見る**前に**要否を確認するため、変換後の値が呼び出し元へ届いたら実装のバグ。
#[derive(Debug, Clone, Copy)]
pub(crate) struct NeedsRestart;

impl From<NeedsRestart> for AppError {
    fn from(_: NeedsRestart) -> Self {
        Self::InternalError(
            "lock declaration escaped the write retry loop (this is a bug in the tikv backend)"
                .to_string(),
        )
    }
}

/// TiKV が「やり直せば通る」種類の失敗を返したか。
///
/// tikv-client は競合を呼び出し側へ委ねるので、ここで拾って「書き込みは待たされても失敗
/// しない」という LMDB の性質を保つ。Debug 文字列の部分一致で判定しないのは、利用者の名前が
/// たまたま一致したり文言変更で黙って 500 に化けたりするため。
fn is_retryable(err: &tikv_client::Error) -> bool {
    use tikv_client::Error;

    match err {
        // 寿命切れ（`commit_ts_expired` / `txn_not_found`）は何もコミットされていない。
        Error::KeyError(key_error) => {
            key_error.conflict.is_some()
                || key_error.deadlock.is_some()
                || !key_error.retryable.is_empty()
                || key_error.commit_ts_expired.is_some()
                || key_error.txn_not_found.is_some()
        }
        // 「今その相手が握っている」だけ。拾い漏らすと**混雑時だけ 500 になる**。
        Error::ResolveLockError(_) => true,
        Error::MultipleKeyErrors(errors) | Error::ExtractedErrors(errors) => {
            errors.iter().any(is_retryable)
        }
        Error::PessimisticLockError { inner, .. } => is_retryable(inner),
        // `UndeterminedError` はコミットの成否が不明なので、やり直すと二重適用になりうる。
        _ => false,
    }
}

/// [`is_retryable`] と分けるのは、やり直す前に 1PC を諦めさせる必要があるため。
fn is_one_pc_failure(err: &tikv_client::Error) -> bool {
    matches!(err, tikv_client::Error::OnePcFailure)
}

/// 複製してもクラスタへの接続は共有される。
#[derive(Clone)]
pub struct TikvDb {
    pub(super) client: Arc<TransactionClient>,
}

/// 保持中のロック。中身が [`Option`] なのは、[`Drop`] でも返しにいけるようにするため。
struct LockGuard {
    txn: Option<Transaction>,
}

impl LockGuard {
    /// まとめて渡すのは、`lock_keys` がリージョンごとに 1 リクエストへ束ねるため。束ねる以上
    /// 呼び出しの**内部**の順は保証されないので、デッドロックは [`is_retryable`] が拾う。
    async fn acquire(
        client: &TransactionClient,
        keys: &BTreeSet<Vec<u8>>,
    ) -> Result<Self, tikv_client::Error> {
        let mut txn = client.begin_with_options(lock_options()).await?;
        if let Err(e) = txn.lock_keys(keys.iter().cloned()).await {
            let _ = txn.rollback().await;
            return Err(e);
        }
        Ok(Self { txn: Some(txn) })
    }

    /// 保持していれば解放する。空集合の試行では取っていないので何もしない。
    async fn release(guard: Option<Self>) {
        if let Some(mut guard) = guard
            && let Some(mut txn) = guard.txn.take()
        {
            let _ = txn.rollback().await;
        }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if let Some(txn) = self.txn.take() {
            rollback_in_background(txn);
        }
    }
}

/// コミットし、**失敗したらロックを返してから**そのエラーを返す。
///
/// [`tikv_client::Transaction::commit`] は失敗しても巻き戻さず、prewrite が途中まで書いたロックが
/// TTL 切れまで居座る。ただし **`UndeterminedError` だけは巻き戻さない**（成功しているかも
/// しれないため）。
async fn commit_or_release(txn: &mut Transaction) -> Result<(), tikv_client::Error> {
    match txn.commit().await {
        Ok(_) => Ok(()),
        Err(tikv_client::Error::UndeterminedError(inner)) => {
            Err(tikv_client::Error::UndeterminedError(inner))
        }
        Err(e) => {
            if let Err(rollback_error) = txn.rollback().await {
                // 巻き戻せなくても元の失敗を優先する。ロックは TTL で消える。
                tracing::warn!("failed to release locks after a failed commit: {rollback_error}");
            }
            Err(e)
        }
    }
}

/// 一度もデータへ触れていない試行では開いていないので、何もしない。
async fn rollback(txn: Option<Transaction>) {
    if let Some(mut txn) = txn {
        let _ = txn.rollback().await;
    }
}

/// `R` は読み取り元。通常の読み取りは [`Snapshot`]、書き込み上の読み取りは `kv::LazyTxn`。
pub struct TikvRead<'a, R = Snapshot> {
    pub(crate) txn: kv::Readers<R>,
    /// ストレージのハンドルより長生きしないことを型で示すだけ。
    pub(crate) _db: PhantomData<&'a TikvDb>,
}

impl TikvRead<'_, Snapshot> {
    /// トランザクションを開かないのが要点。開始と rollback の 2 往復が要らず、タイムスタンプを
    /// 手元に持てるので同じ断面を追加で開いて並行に読める（[`kv::Readers`]）。
    pub(crate) fn at(client: Arc<TransactionClient>, ts: tikv_client::Timestamp) -> Self {
        Self {
            txn: kv::Readers::fanned_out(client, ts),
            _db: PhantomData,
        }
    }
}

pub struct TikvWrite<'a> {
    pub(crate) txn: kv::Readers<LazyTxn>,
    pub(crate) _db: PhantomData<&'a TikvDb>,
    /// [`BTreeSet`] なので取得はキーのバイト昇順、つまり [`LockScope`] の順になる。
    /// これがロック階層の全順序になり、待ちグラフに循環ができない。
    held: BTreeSet<Vec<u8>>,
    /// 宣言されたが、まだ保持していないロック。空でなければこの試行は巻き戻す。
    missing: BTreeSet<Vec<u8>>,
    /// 読んだ木の形が古かった。
    stale: bool,
}

impl TikvWrite<'_> {
    /// この操作が触る範囲を宣言する。**実際にデータへ触れる前**に呼ぶこと。
    ///
    /// 「ロックを取って初めて対象が判る」操作があるので、呼び出し前に集合を列挙できない。
    /// 足りなければ巻き戻し、揃えた状態でやり直す。戻り値にしてあるのは宣言忘れの検出用。
    pub(crate) fn require_lock(&mut self, scope: LockScope, id: &[u8]) -> Result<(), NeedsRestart> {
        let key = keys::lock(scope, id);
        if self.held.contains(&key) {
            return Ok(());
        }
        self.missing.insert(key);
        Err(NeedsRestart)
    }

    /// 1 つずつ宣言すると 1 回の試行で 1 つしか集められないので、まとめて宣言する。
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

    /// 読み直しても `get` は `start_ts` を読むので、新しい試行でないと前進しない。
    pub(crate) fn mark_stale(&mut self) -> NeedsRestart {
        self.stale = true;
        NeedsRestart
    }

    fn needs_restart(&self) -> bool {
        !self.missing.is_empty() || self.stale
    }
}

impl Storage for TikvDb {
    type Read<'a> = TikvRead<'a, Snapshot>;
    type Write<'a> = TikvWrite<'a>;
    type QuerySnapshot = tikv_client::Timestamp;

    #[tracing::instrument(skip_all)]
    async fn query_snapshot(&self) -> Result<Self::QuerySnapshot, AppError> {
        self.client
            .current_timestamp()
            .await
            .map_err(AppError::from)
    }

    #[tracing::instrument(skip_all)]
    async fn read<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a Self::Read<'b>) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static,
    {
        // ロックを取らないので、書き込みをブロックせず書き込みにもブロックされない。
        let start = std::time::Instant::now();
        let ts = self.client.current_timestamp().await?;
        let r = TikvRead::at(self.client.clone(), ts);
        let result = f(&r).await;
        crate::telemetry::metrics::read_transaction(start.elapsed().as_secs_f64());
        result
    }

    /// リトライは競合が前提の制御フローなので、試行ごとにスパンは張らない
    /// （張ると輻輳時に大量の子スパンが生まれる）。代わりに [`WriteOutcome`]
    /// 別の計器へ数える。全体の所要時間はリトライぶんも含めて 1 回だけ記録する。
    ///
    /// [`WriteOutcome`]: crate::telemetry::metrics::WriteOutcome
    #[tracing::instrument(skip_all)]
    async fn write<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a mut Self::Write<'b>) -> Result<T, AppError>
            + Clone
            + Send
            + 'static,
        T: Send + 'static,
    {
        use crate::telemetry::metrics::{WriteOutcome, write_attempt};
        let start = std::time::Instant::now();
        let outcome = self.write_retrying(f).await;
        crate::telemetry::metrics::write_transaction(start.elapsed().as_secs_f64());
        write_attempt(if outcome.is_ok() {
            WriteOutcome::Committed
        } else {
            WriteOutcome::Failed
        });
        outcome
    }
}

impl TikvDb {
    async fn write_retrying<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a mut TikvWrite<'b>) -> Result<T, AppError>
            + Clone
            + Send
            + 'static,
        T: Send + 'static,
    {
        use crate::telemetry::metrics::{WriteOutcome, write_attempt};

        let mut locks: BTreeSet<Vec<u8>> = BTreeSet::new();
        // 数えるのは競合だけ。ロック不足での巻き戻しは相手を待っているわけではない。
        let mut retry = Retry::default();
        // 試行をまたいで持ち回らないと、断られ続ける構成で上限まで失敗し続ける。
        let mut one_pc = true;

        for _ in 0..MAX_ATTEMPTS {
            // 空の `lock_keys` は何もしないのに、開くだけで PD から ts を取ることになる。
            let guard = if locks.is_empty() {
                None
            } else {
                match LockGuard::acquire(&self.client, &locks).await {
                    Ok(guard) => Some(guard),
                    Err(e) if is_retryable(&e) => {
                        retry.back_off(AppError::from(e)).await;
                        continue;
                    }
                    Err(e) => return Err(AppError::from(e)),
                }
            };

            // start_ts がロック取得より後になるので、前任者のコミットが必ず見える。
            let mut w = TikvWrite {
                txn: kv::Readers::new(LazyTxn::new(self.client.clone(), one_pc)),
                _db: PhantomData,
                held: locks.clone(),
                missing: BTreeSet::new(),
                stale: false,
            };

            // やり直しに備えて複製を渡す。`f` 自体は次の試行のために残す。
            let result = f.clone()(&mut w).await;
            // **結果より先**に確認する。クロージャが握り潰して `Ok` を返しても捨てられる。
            let restart = w.needs_restart();
            let newly_required = std::mem::take(&mut w.missing);
            let was_stale = w.stale;
            let mut lazy = w.txn.into_inner();

            // 捨てる試行では送らない。送らなければロックも MVCC のバージョンも作られない。
            let flushed = if restart || result.is_err() {
                Ok(())
            } else {
                lazy.flush().await
            };

            // 競合はクロージャの内側で起きるので、失敗の分類はトランザクションが覚えている。
            let conflicted = lazy.conflicted();
            let txn = lazy.into_opened();

            if restart {
                rollback(txn).await;
                LockGuard::release(guard).await;
                locks.extend(newly_required);
                // 古い読みが原因のときだけ待つ。ロック不足なら次の周回は必ず前進する。
                if was_stale {
                    write_attempt(WriteOutcome::Stale);
                    let reason = AppError::Conflict(
                        "read a stale view of the shard tree; retrying with a newer snapshot"
                            .to_string(),
                    );
                    retry.back_off(reason).await;
                } else {
                    write_attempt(WriteOutcome::LockDeclared);
                }
                continue;
            }

            if let Err(e) = flushed {
                rollback(txn).await;
                LockGuard::release(guard).await;
                if is_retryable(&e) {
                    write_attempt(WriteOutcome::Conflict);
                    retry.back_off(AppError::from(e)).await;
                    continue;
                }
                return Err(AppError::from(e));
            }

            let outcome = match result {
                Ok(value) => match txn {
                    Some(mut txn) => commit_or_release(&mut txn).await.map(|_| value),
                    // 何も書かずに終わった（読み取りだけ、あるいは即座に確定した）。
                    None => Ok(value),
                },
                Err(app_err) => {
                    rollback(txn).await;
                    LockGuard::release(guard).await;
                    if conflicted {
                        // 競合で弾かれただけなので、新しいトランザクションでやり直す。
                        write_attempt(WriteOutcome::Conflict);
                        retry.back_off(app_err).await;
                        continue;
                    }
                    return Err(app_err);
                }
            };

            LockGuard::release(guard).await;

            match outcome {
                Ok(value) => return Ok(value),
                // 競合ではないので待たず、2PC で即やり直す。
                Err(e) if is_one_pc_failure(&e) => {
                    // 書き込みごとに発火しうるので trace に留める。
                    tracing::trace!("TiKV declined one-phase commit; retrying with two-phase");
                    one_pc = false;
                    retry.note(AppError::from(e));
                    continue;
                }
                Err(e) if is_retryable(&e) => {
                    write_attempt(WriteOutcome::Conflict);
                    retry.back_off(AppError::from(e)).await;
                    continue;
                }
                Err(e) => return Err(AppError::from(e)),
            }
        }

        // `last_error` が無いのはロック宣言が収束しなかったとき（実装のバグ）。
        Err(retry.last_error.unwrap_or_else(|| {
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

    /// この並びが崩れるとデッドロック回避の前提が壊れる。
    #[test]
    fn lock_scopes_sort_into_the_hierarchy_order() {
        let db = keys::lock(LockScope::Database, b"z-database");
        let user = keys::lock(LockScope::User, b"a-user");

        // id の中身に関わらず、スコープの順序が先に効く。
        assert!(db < user);

        let ordered: Vec<_> = BTreeSet::from([user.clone(), db.clone()])
            .into_iter()
            .collect();
        assert_eq!(ordered, vec![db, user]);
    }

    /// 揃った間隔でやり直すと、ぶつかった相手と足並みが揃ったままになる。
    #[test]
    fn retry_backoff_grows_within_bounds_and_varies() {
        // 上限を超えない（揺らぎの上振れを含めて）。
        let ceiling =
            RETRY_BACKOFF_MAX + RETRY_BACKOFF_MAX * RETRY_BACKOFF_JITTER_PERCENT as u32 / 100;
        for attempt in 0..32 {
            assert!(
                retry_backoff(attempt) <= ceiling,
                "attempt={attempt} で上限を超えた"
            );
        }

        // 十分に伸びた段階では、毎回同じ値にならない。
        let samples: std::collections::BTreeSet<_> = (0..32).map(|_| retry_backoff(16)).collect();
        assert!(
            samples.len() > 1,
            "揺らぎが効いておらず、待ち時間が毎回同じになっている"
        );

        // 揺らぎは ±20% の範囲に収まる。
        let base = RETRY_BACKOFF_MAX.as_millis() as u64;
        let span = base * RETRY_BACKOFF_JITTER_PERCENT / 100;
        for sample in samples {
            let millis = sample.as_millis() as u64;
            assert!(
                millis >= base - span && millis <= base + span,
                "揺らぎが想定範囲を外れている: {millis}ms"
            );
        }
    }

    /// 分類が漏れていると**混雑したときだけ 500 が返る**。
    #[test]
    fn contention_failures_are_classified_as_retryable() {
        use tikv_client::Error;

        assert!(
            is_retryable(&Error::ResolveLockError(Vec::new())),
            "ロック解決の失敗がやり直し対象から漏れている"
        );
        // 入れ子（リージョンを跨いだ prewrite でまとめられた場合）でも拾う。
        assert!(is_retryable(&Error::ExtractedErrors(vec![
            Error::ResolveLockError(Vec::new()),
        ])));

        let expired = Error::KeyError(Box::new(tikv_client::ProtoKeyError {
            commit_ts_expired: Some(Default::default()),
            ..Default::default()
        }));
        assert!(
            is_retryable(&expired),
            "TTL 切れ（commit_ts_expired）がやり直し対象から漏れている"
        );
        let vanished = Error::KeyError(Box::new(tikv_client::ProtoKeyError {
            txn_not_found: Some(Default::default()),
            ..Default::default()
        }));
        assert!(
            is_retryable(&vanished),
            "巻き戻された試行（txn_not_found）がやり直し対象から漏れている"
        );

        // 競合でも寿命切れでもない `KeyError` は拾わない（無限にやり直さないため）。
        let other = Error::KeyError(Box::new(tikv_client::ProtoKeyError {
            already_exist: Some(Default::default()),
            ..Default::default()
        }));
        assert!(!is_retryable(&other));

        // コミットの成否が不明なものはやり直してはならない（二重適用になりうる）。
        assert!(!is_retryable(&Error::UndeterminedError(Box::new(
            Error::ResolveLockError(Vec::new())
        ))));

        // 1PC 拒否は競合ではないので、こちらの分類には入れない。
        assert!(!is_retryable(&Error::OnePcFailure));
        assert!(is_one_pc_failure(&Error::OnePcFailure));
    }

    /// ロック専用が楽観になると排他が黙って消えるが、競合を起こさない単体テストは通ってしまう。
    /// 気づくのが本番になるので型として固定する。
    #[test]
    fn lock_transactions_stay_pessimistic() {
        // 悲観／楽観の別は外から読めないので、既知の構成と突き合わせる。
        let pessimistic = TransactionOptions::new_pessimistic().drop_check(CheckLevel::Warn);
        assert_eq!(lock_options(), pessimistic, "ロック専用は悲観であること");

        // 作業側は今も悲観だが、それに依存して同一視してはならない。
        let optimistic = TransactionOptions::new_optimistic().drop_check(CheckLevel::Warn);
        assert_ne!(
            lock_options(),
            optimistic,
            "ロック専用が楽観になっている。lock_keys が RPC を出さなくなり排他が消える"
        );

        assert_ne!(
            write_options(true),
            write_options(false),
            "try_one_pc が反映されていない"
        );
    }

    #[test]
    fn restart_marker_is_labelled_as_a_bug_when_converted() {
        let err: AppError = NeedsRestart.into();
        let AppError::InternalError(message) = err else {
            panic!("NeedsRestart must convert into InternalError");
        };
        assert!(message.contains("bug"), "実装バグと判る文言であること");
    }
}
