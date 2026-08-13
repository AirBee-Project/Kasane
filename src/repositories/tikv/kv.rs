//! トランザクション上の素の KV 操作と、シャード値のフレーム。
//!
//! 読み取りは [`TikvRead`](super::TikvRead) と [`TikvWrite`](super::TikvWrite) の
//! どちらからも必要なので、読み取り元だけを受け取る自由関数として置いている。

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use kasane_logic::FlexId;
use rustc_hash::FxHashSet;
pub(super) use tikv_client::proto::kvrpcpb::Mutation;
use tikv_client::proto::kvrpcpb::{Assertion, Op};
use tikv_client::{BoundRange, Snapshot, Timestamp, Transaction, TransactionClient};
use tokio::sync::{Mutex, MutexGuard};

use super::keys;
use crate::error::AppError;
use crate::models::id::TableId;
use crate::repositories::encoding::shard_entry::ShardEntry;

/// 1 回のスキャンで取り出す件数。TiKV は 1 リクエストの上限があるため分割して読む。
const SCAN_BATCH: u32 = 512;

/// 1 リクエストへ載せるキー数の上限。
///
/// 同じテーブルのシャードキーは連続していてひとつのリージョンに集まりやすく、そのまま渡すと
/// gRPC のメッセージサイズ上限に触れる。呼び出し側で数を見積もるのは難しい（木の形と対象
/// 領域の広さで変わる）ので、ここで一律に区切る。
///
/// 分割してもキーの昇順は保たれるので、デッドロック回避の前提は崩れない。
const BATCH_KEYS: usize = 1024;

/// 1 チャンクが既にリージョン単位で内部並行化されているので、ここを大きくしても
/// 増えるのは同時に飛ぶリクエスト数と手元に載る応答の量だけ。
const MAX_FANOUT: usize = 8;

/// スナップショット読み取りができるもの（[`Transaction`] / [`Snapshot`] / [`LazyTxn`]）。
///
/// `pub` なのは [`TikvRead`](super::TikvRead) の型引数の境界に現れるためだけ。
/// `kv` モジュール自体が非公開なので外からは名指しできない。
// `async fn` の Future に `Send` を課さないのは `traits/storage.rs` と同じ理由。
#[allow(async_fn_in_trait)]
pub trait Reader {
    async fn read_one(&mut self, key: Vec<u8>) -> Result<Option<Vec<u8>>, tikv_client::Error>;

    async fn read_many(
        &mut self,
        keys: Vec<Vec<u8>>,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error>;

    async fn read_range(
        &mut self,
        range: BoundRange,
        limit: u32,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error>;

    async fn read_range_keys(
        &mut self,
        range: BoundRange,
        limit: u32,
    ) -> Result<Vec<Vec<u8>>, tikv_client::Error>;

    /// `start`（含む）〜`end`（排他、`None` なら終端まで）の**まだ送っていない変更**を
    /// キー昇順で返す（`None` は削除）。溜める仕組みを持たない読み取り元では常に空。
    ///
    /// 点読みと違い、範囲読みは自分の変更をその場で重ねられない。読み切る関数は
    /// 取得件数が上限に満たないことを終端の判定に使うので、途中で件数を増減させると
    /// 読み切る前に止まったり同じ場所を読み直したりする。
    fn staged_range(&self, _start: &[u8], _end: Option<&[u8]>) -> Vec<(Vec<u8>, Option<Vec<u8>>)> {
        Vec::new()
    }
}

/// [`Transaction`] と [`Snapshot`] は読み取り系のシグネチャが完全に一致する。
macro_rules! impl_reader {
    ($target:ty) => {
        impl Reader for $target {
            async fn read_one(
                &mut self,
                key: Vec<u8>,
            ) -> Result<Option<Vec<u8>>, tikv_client::Error> {
                self.get(key).await
            }

            async fn read_many(
                &mut self,
                keys: Vec<Vec<u8>>,
            ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error> {
                Ok(self
                    .batch_get(keys)
                    .await?
                    .map(|kv| (Vec::from(kv.0), kv.1))
                    .collect())
            }

            async fn read_range(
                &mut self,
                range: BoundRange,
                limit: u32,
            ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error> {
                Ok(self
                    .scan(range, limit)
                    .await?
                    .map(|kv| (Vec::from(kv.0), kv.1))
                    .collect())
            }

            async fn read_range_keys(
                &mut self,
                range: BoundRange,
                limit: u32,
            ) -> Result<Vec<Vec<u8>>, tikv_client::Error> {
                Ok(self.scan_keys(range, limit).await?.map(Vec::from).collect())
            }
        }
    };
}

impl_reader!(Transaction);
impl_reader!(Snapshot);

/// 読み取り／書き込みの実体と、並行読みのための追加スナップショット。
///
/// [`Reader`] は `&mut self` を要求するので、1 つの実体ではチャンクを逐次にしか読めず、そこが
/// 往復の直列鎖になる。断面がタイムスタンプで決まる読み取り（[`fanned_out`](Self::fanned_out)）
/// なら、同じ断面のスナップショットをいくつでも開ける。
///
/// 書き込みトランザクション上の読み取りには持たせない。そちらは**まだ送っていない自分の変更**
/// を重ねて返す必要があり、別の実体から読むとその重ね合わせが効かない。
pub struct Readers<R> {
    primary: Mutex<R>,
    fanout: Option<Fanout>,
}

struct Fanout {
    client: Arc<TransactionClient>,
    ts: Timestamp,
}

impl Fanout {
    fn open(&self) -> Snapshot {
        self.client.snapshot(self.ts.clone(), super::read_options())
    }
}

impl<R> Readers<R> {
    /// 並行読みをしない読み取り元。
    pub(super) fn new(reader: R) -> Self {
        Self {
            primary: Mutex::new(reader),
            fanout: None,
        }
    }

    pub(super) fn into_inner(self) -> R {
        self.primary.into_inner()
    }

    async fn lock(&self) -> MutexGuard<'_, R> {
        self.primary.lock().await
    }
}

impl Readers<Snapshot> {
    pub(super) fn fanned_out(client: Arc<TransactionClient>, ts: Timestamp) -> Self {
        let primary = client.snapshot(ts.clone(), super::read_options());
        Self {
            primary: Mutex::new(primary),
            fanout: Some(Fanout { client, ts }),
        }
    }
}

/// 書き込み側のトランザクション。**最初に実際へ触れるまで開かない。**
///
/// ロックを宣言しただけで巻き戻る 1 周目が、何もせずに捨てるためだけに PD からタイムスタンプ
/// を取ることを避ける。開くのが遅れても「ロックを取ってから `start_ts` を確定させる」という
/// 不変条件は損なわれない（むしろ強まる）。
pub struct LazyTxn {
    client: Arc<TransactionClient>,
    txn: Option<Transaction>,
    one_pc: bool,
    /// まだ TiKV へ送っていない変更（`None` は削除）。コミット直前に 1 回だけ流す。
    ///
    /// 呼び出しのたびに `batch_mutate` を投げると、触れたリーフの数だけリクエストが
    /// 直列に積み上がる。[`BTreeMap`] なのは、反復順（キーのバイト昇順）で送れば
    /// リージョン跨ぎのアクセスが減り、同じキーへの重複書き込みも畳まれるため。
    ///
    /// **読み取りに自分の書き込みを見せる**用途も兼ねる（[`LazyTxn`] の `Reader` 実装）。
    pending: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    /// ロックの取得はこのトランザクションの**内側**で起きるので、競合は `Storage::write`
    /// から見ると「クロージャが返したアプリケーションエラー」になる。判断できるよう記録する。
    conflicted: bool,
}

impl LazyTxn {
    pub(super) fn new(client: Arc<TransactionClient>, one_pc: bool) -> Self {
        Self {
            client,
            txn: None,
            one_pc,
            pending: BTreeMap::new(),
            conflicted: false,
        }
    }

    /// **このトランザクションが出す失敗は読み書きを問わずここを通すこと。**
    ///
    /// 通し忘れると `Storage::write` から「ただのアプリケーションエラー」に見え、やり直されずに
    /// 500 になる。木の降下も他者のロックに当たるので、読み取り側も対象。
    fn note(&mut self, err: tikv_client::Error) -> tikv_client::Error {
        if super::is_retryable(&err) {
            self.conflicted = true;
        }
        err
    }

    pub(super) fn conflicted(&self) -> bool {
        self.conflicted
    }

    async fn open(&mut self) -> Result<&mut Transaction, tikv_client::Error> {
        if self.txn.is_none() {
            self.txn = Some(
                self.client
                    .begin_with_options(super::write_options(self.one_pc))
                    .await?,
            );
        }
        Ok(self.txn.as_mut().expect("直前に開いた"))
    }

    /// 取り出した後は [`Drop`] の対象が無くなるので、後始末は呼び出し側の責任になる。
    pub(super) fn into_opened(mut self) -> Option<Transaction> {
        self.txn.take()
    }

    fn stage(&mut self, mutations: impl IntoIterator<Item = Mutation>) {
        for m in mutations {
            let value = (m.op != Op::Del as i32).then_some(m.value);
            self.pending.insert(m.key, value);
        }
    }

    /// 溜めた変更をまとめて送り、悲観ロックを取る。**コミットの直前に 1 度だけ呼ぶ。**
    ///
    /// 全キーを 1 回の `batch_mutate` へ載せるので、ロック取得は「TSO 1 回 + PessimisticLock
    /// 1 回」で済む。捨てる試行では呼ばない――溜めたまま drop すれば、ロックも MVCC の
    /// バージョンも一切作られないまま消える。
    pub(super) async fn flush(&mut self) -> Result<(), tikv_client::Error> {
        if self.pending.is_empty() {
            return Ok(());
        }
        // ここから先の持ち主は TiKV 側のバッファ。両方が同じバイト列を抱える瞬間を作らない。
        let staged = std::mem::take(&mut self.pending);
        let mut mutations: Vec<Mutation> = staged
            .into_iter()
            .map(|(key, value)| match value {
                Some(value) => put_mutation(key, value),
                None => delete_mutation(key),
            })
            .collect();

        while !mutations.is_empty() {
            let rest = mutations.split_off(mutations.len().min(BATCH_KEYS));
            let result = self.open().await?.batch_mutate(mutations).await;
            result.map_err(|e| self.note(e))?;
            mutations = rest;
        }
        Ok(())
    }

    /// キーをロックし、**そのロック時点の**値を返す。
    ///
    /// 「ロックしてから `get`」だと、`get` が `start_ts` を読むためロック取得までの間に他者が
    /// コミットした変更を見落とす。`batch_get_for_update` は取り直した `for_update_ts` の値を
    /// 返すので read-modify-write が安全に組める。
    ///
    /// 存在しないキーも**ロックされる**（結果には現れない）。空の領域を他者が埋めたり親へ
    /// 畳んだりするのを防ぐのに、この性質を使っている。
    ///
    /// 応答には自分の未送信の変更が映らないので手元の台帳を優先させる。これがないと、1 回の
    /// クロージャで同じリーフを 2 度触る操作が自分の書き込みを失う。
    async fn lock_and_read(
        &mut self,
        mut keys: Vec<Vec<u8>>,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, tikv_client::Error> {
        // キーは昇順で渡ってくる。分割しても順序は保たれる。
        let mut locked: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        let all_keys = keys.clone();
        while !keys.is_empty() {
            let rest = keys.split_off(keys.len().min(BATCH_KEYS));
            let result = self.open().await?.batch_get_for_update(keys).await;
            let pairs = result.map_err(|e| self.note(e))?;
            locked.extend(pairs.into_iter().map(|kv| (Vec::from(kv.0), kv.1)));
            keys = rest;
        }

        for key in all_keys {
            match self.pending.get(&key) {
                Some(Some(value)) => {
                    locked.insert(key, value.clone());
                }
                Some(None) => {
                    locked.remove(&key);
                }
                None => {}
            }
        }
        Ok(locked)
    }
}

impl Drop for LazyTxn {
    fn drop(&mut self) {
        if let Some(txn) = self.txn.take() {
            super::rollback_in_background(txn);
        }
    }
}

/// 読み取りは**未送信の変更を先に見る**。そして**失敗は必ず [`note`](LazyTxn::note) を通す**。
///
/// 前者は「書いてから読む」（データベースを作ってすぐその中にテーブルを作る等）を成立させる
/// ため。後者は競合の取りこぼしを防ぐためで、木の降下も他者のロックに当たりうる。
impl Reader for LazyTxn {
    async fn read_one(&mut self, key: Vec<u8>) -> Result<Option<Vec<u8>>, tikv_client::Error> {
        if let Some(staged) = self.pending.get(&key) {
            return Ok(staged.clone());
        }
        let result = self.open().await?.read_one(key).await;
        result.map_err(|e| self.note(e))
    }

    async fn read_many(
        &mut self,
        keys: Vec<Vec<u8>>,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error> {
        // 自分が触ったキーは手元で答え、残りだけを TiKV へ聞く。
        let mut staged = Vec::new();
        let mut ask = Vec::with_capacity(keys.len());
        for key in keys {
            match self.pending.get(&key) {
                Some(Some(value)) => staged.push((key, value.clone())),
                // 自分で消したキーは結果に現れない。
                Some(None) => {}
                None => ask.push(key),
            }
        }

        let mut out = if ask.is_empty() {
            Vec::new()
        } else {
            let result = self.open().await?.read_many(ask).await;
            result.map_err(|e| self.note(e))?
        };
        out.append(&mut staged);
        Ok(out)
    }

    /// 範囲読みは生のまま返す。重ねるのは呼び出し側（[`staged_range`](Reader::staged_range)）。
    async fn read_range(
        &mut self,
        range: BoundRange,
        limit: u32,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error> {
        let result = self.open().await?.read_range(range, limit).await;
        result.map_err(|e| self.note(e))
    }

    async fn read_range_keys(
        &mut self,
        range: BoundRange,
        limit: u32,
    ) -> Result<Vec<Vec<u8>>, tikv_client::Error> {
        let result = self.open().await?.read_range_keys(range, limit).await;
        result.map_err(|e| self.note(e))
    }

    fn staged_range(&self, start: &[u8], end: Option<&[u8]>) -> Vec<(Vec<u8>, Option<Vec<u8>>)> {
        use std::ops::Bound;

        let lower = Bound::Included(start.to_vec());
        let upper = match end {
            Some(end) => Bound::Excluded(end.to_vec()),
            None => Bound::Unbounded,
        };
        self.pending
            .range((lower, upper))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}

// --- 素の KV 操作 ---

pub(super) async fn get<R: Reader>(
    txn: &Readers<R>,
    key: Vec<u8>,
) -> Result<Option<Vec<u8>>, AppError> {
    let mut txn = txn.lock().await;
    txn.read_one(key).await.map_err(AppError::from)
}

// 積むだけなので**失敗しない**。台帳は [`BTreeMap`] なので積む順序は送信順に影響しない。

pub(super) async fn stage(txn: &Readers<LazyTxn>, mutations: impl IntoIterator<Item = Mutation>) {
    txn.lock().await.stage(mutations);
}

pub(super) async fn put(txn: &Readers<LazyTxn>, key: Vec<u8>, value: Vec<u8>) {
    stage(txn, [put_mutation(key, value)]).await;
}

pub(super) async fn delete(txn: &Readers<LazyTxn>, key: Vec<u8>) {
    stage(txn, [delete_mutation(key)]).await;
}

pub(super) async fn put_many(
    txn: &Readers<LazyTxn>,
    entries: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
) {
    stage(
        txn,
        entries
            .into_iter()
            .map(|(key, value)| put_mutation(key, value)),
    )
    .await;
}

pub(super) async fn delete_many(txn: &Readers<LazyTxn>, keys: impl IntoIterator<Item = Vec<u8>>) {
    stage(txn, keys.into_iter().map(delete_mutation)).await;
}

pub(super) fn put_mutation(key: Vec<u8>, value: Vec<u8>) -> Mutation {
    Mutation {
        op: Op::Put as i32,
        key,
        value,
        assertion: Assertion::None as i32,
    }
}

pub(super) fn delete_mutation(key: Vec<u8>) -> Mutation {
    Mutation {
        op: Op::Del as i32,
        key,
        value: Vec::new(),
        assertion: Assertion::None as i32,
    }
}

/// 存在しないキーは結果に現れない。
///
/// チャンクは互いに独立なので、追加スナップショットを開ける読み取り元では並行に投げる。
pub(super) async fn batch_get<R: Reader>(
    txn: &Readers<R>,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    if keys.len() > BATCH_KEYS
        && let Some(fanout) = &txn.fanout
    {
        return batch_get_fanned_out(fanout, keys).await;
    }

    let mut txn = txn.lock().await;
    // 収まる場合はそのまま渡す（`chunks(..).to_vec()` だとキー列を丸ごと複製する）。
    if keys.len() <= BATCH_KEYS {
        return txn.read_many(keys).await.map_err(AppError::from);
    }

    let mut rest = keys;
    let mut out = Vec::new();
    while !rest.is_empty() {
        let tail = rest.split_off(rest.len().min(BATCH_KEYS));
        out.extend(txn.read_many(rest).await?);
        rest = tail;
    }
    Ok(out)
}

/// チャンクごとに独立したスナップショットを開き、[`MAX_FANOUT`] 本まで並行に読む。
///
/// **失敗しても飛んでいる RPC を打ち切らない。** [`JoinSet`](tokio::task::JoinSet) は drop すると
/// 中のタスクを abort するので、1 本の失敗で `?` を返すと応答待ちの兄弟が捨てられ gRPC
/// ストリームが RST で切れる。これは混んでいるときほど増えるので、詰まりかけたクラスタへ
/// こちらからキャンセルを浴びせる形になる。
async fn batch_get_fanned_out(
    fanout: &Fanout,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    type ChunkResult = Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error>;

    let mut chunks = keys.chunks(BATCH_KEYS).map(<[Vec<u8>]>::to_vec);
    let mut running: tokio::task::JoinSet<ChunkResult> = tokio::task::JoinSet::new();
    let mut out = Vec::new();
    let mut failure: Option<AppError> = None;

    loop {
        // 失敗が判ったら新しいチャンクは投げない。ただし投げ終えたぶんは待つ。
        while failure.is_none()
            && running.len() < MAX_FANOUT
            && let Some(chunk) = chunks.next()
        {
            let mut snapshot = fanout.open();
            running.spawn(async move {
                Ok(snapshot
                    .batch_get(chunk)
                    .await?
                    .map(|kv| (Vec::from(kv.0), kv.1))
                    .collect())
            });
        }

        let Some(joined) = running.join_next().await else {
            break;
        };

        match joined
            .map_err(|e| AppError::InternalError(format!("batch get task: {e}")))
            .and_then(|result| result.map_err(AppError::from))
        {
            Ok(pairs) if failure.is_none() => out.extend(pairs),
            Ok(_) => {}
            Err(e) => {
                failure.get_or_insert(e);
            }
        }
    }

    match failure {
        Some(e) => Err(e),
        None => Ok(out),
    }
}

fn range_from(start: Vec<u8>, end: Option<&[u8]>) -> BoundRange {
    match end {
        Some(end) => BoundRange::from(start..end.to_vec()),
        None => BoundRange::from(start..),
    }
}

/// 読み切ったなら `None`、続きがあるなら次のバッチの開始位置。
///
/// **判定は生の取得件数で行う。** 未送信の変更を重ねるのは読み切ったあとで、途中で件数を
/// 増減させると読み切る前に止まったり同じ場所を読み直したりする。
fn next_cursor(last_key: &[u8], fetched: usize) -> Option<Vec<u8>> {
    if fetched < SCAN_BATCH as usize {
        return None;
    }
    let mut cursor = last_key.to_vec();
    cursor.push(0);
    Some(cursor)
}

/// プレフィックスに一致する全キーと値を取り出す。
pub(super) async fn scan_prefix<R: Reader>(
    txn: &Readers<R>,
    prefix: &[u8],
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    let end = keys::prefix_end(prefix);
    let mut cursor = Some(prefix.to_vec());
    let mut out: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();

    while let Some(start) = cursor {
        let batch = {
            let mut txn = txn.lock().await;
            txn.read_range(range_from(start, end.as_deref()), SCAN_BATCH)
                .await?
        };
        let fetched = batch.len();
        out.extend(batch);
        cursor = out.last().and_then(|(key, _)| next_cursor(key, fetched));
    }

    let staged = txn.lock().await.staged_range(prefix, end.as_deref());
    if staged.is_empty() {
        return Ok(out);
    }
    // キー順で返す約束は保つ。
    let mut merged: BTreeMap<Vec<u8>, Vec<u8>> = out.into_iter().collect();
    for (key, value) in staged {
        match value {
            Some(value) => merged.insert(key, value),
            None => merged.remove(&key),
        };
    }
    Ok(merged.into_iter().collect())
}

/// `start`（含む）から `end`（排他、`None` なら終端まで）のキーだけを取り出す。
pub(super) async fn scan_keys_range<R: Reader>(
    txn: &Readers<R>,
    start: Vec<u8>,
    end: Option<Vec<u8>>,
) -> Result<Vec<Vec<u8>>, AppError> {
    let mut cursor = Some(start.clone());
    let mut out: Vec<Vec<u8>> = Vec::new();

    while let Some(from) = cursor {
        let batch = {
            let mut txn = txn.lock().await;
            txn.read_range_keys(range_from(from, end.as_deref()), SCAN_BATCH)
                .await?
        };
        let fetched = batch.len();
        out.extend(batch);
        cursor = out.last().and_then(|key| next_cursor(key, fetched));
    }

    let staged = txn.lock().await.staged_range(&start, end.as_deref());
    if staged.is_empty() {
        return Ok(out);
    }
    // キー順で返す約束は保つ。
    let mut merged: BTreeSet<Vec<u8>> = out.into_iter().collect();
    for (key, value) in staged {
        if value.is_some() {
            merged.insert(key);
        } else {
            merged.remove(&key);
        }
    }
    Ok(merged.into_iter().collect())
}

/// プレフィックスに一致する全キーだけを取り出す。
pub(super) async fn scan_prefix_keys<R: Reader>(
    txn: &Readers<R>,
    prefix: &[u8],
) -> Result<Vec<Vec<u8>>, AppError> {
    scan_keys_range(txn, prefix.to_vec(), keys::prefix_end(prefix)).await
}

/// `start`（含む）から `end_inclusive`（含む）までのキーを取り出す。
pub(super) async fn scan_inclusive_keys<R: Reader>(
    txn: &Readers<R>,
    start: Vec<u8>,
    mut end_inclusive: Vec<u8>,
) -> Result<Vec<Vec<u8>>, AppError> {
    // 終端を含めたいので、末尾に 0 を足して排他終端へ変換する。
    end_inclusive.push(0);
    scan_keys_range(txn, start, Some(end_inclusive)).await
}

/// CRC32（u32 LE）。
const FRAME_HEADER_LEN: usize = 4;

/// CRC 検証を通ったシャード値。
///
/// シャード値だけを CRC32 で包むのは、ペイロードの読み出しが `access_unchecked`（構造を
/// 検証しないゼロコピーアクセス）を通るため。その安全条件は「自分が書いたバイト列そのもの」
/// だが、TiKV ではバイト列がネットワーク越しに届き、クラスタは複数インスタンス・複数
/// バージョンで共有されうるのでこの前提が成り立たない。
///
/// 検証はペイロードを 1 度なめるだけでコピーは発生しない（[`entry`](Self::entry) は受信
/// バッファへの借用を返す）ので、ゼロコピーはそのまま保たれる。LMDB 側にこの枠は無い。
pub(super) struct ShardValue {
    framed: Vec<u8>,
}

/// ここを通過したバイト列だけが `access_unchecked` へ渡る。
impl TryFrom<Vec<u8>> for ShardValue {
    type Error = AppError;

    fn try_from(framed: Vec<u8>) -> Result<Self, AppError> {
        if framed.len() < FRAME_HEADER_LEN {
            return Err(AppError::StorageError(
                "shard value is shorter than its frame header".to_string(),
            ));
        }
        let expected = u32::from_le_bytes(
            framed[..FRAME_HEADER_LEN]
                .try_into()
                .expect("FRAME_HEADER_LEN バイトある"),
        );
        let actual = crc32fast::hash(&framed[FRAME_HEADER_LEN..]);
        if actual != expected {
            return Err(AppError::StorageError(format!(
                "shard value failed its integrity check \
                 (crc32 expected {expected:#010x}, found {actual:#010x})"
            )));
        }
        Ok(Self { framed })
    }
}

impl ShardValue {
    /// 検証済みのシャードエントリ（`encoding::shard_entry` の形式）。
    pub(super) fn entry(&self) -> &[u8] {
        &self.framed[FRAME_HEADER_LEN..]
    }
}

fn frame(entry: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME_HEADER_LEN + entry.len());
    out.extend_from_slice(&crc32fast::hash(entry).to_le_bytes());
    out.extend_from_slice(entry);
    out
}

/// シャードの保存を表す変更（本体と件数）を組み立てる。**ネットワークには触れない。**
///
/// シャードは本体（`0x06`）と保持件数（`0x08`）の 2 本のキーで表される。件数はエントリの
/// ヘッダにも入っているが、そちらを読むにはリーフ本体ごと転送することになり、合算するだけの
/// `table_count` には割に合わない。別キーなら集計はシャード数 × 4 バイトで済む。
/// LMDB 側にこのキーは無い（mmap 上をストリームで舐められるので代償が無い）。
///
/// 2 本を必ず一組で返すのが要点で、片方だけ書いて食い違わせないための入口になっている。
/// 送信と分けてあるのは、重い計算（rkyv の復元・直列化）ごと blocking タスクへ移せるように
/// するため（[`super::tree`] を参照）。
pub(super) fn shard_mutations(
    table_id: TableId,
    region: &FlexId,
    entry: &[u8],
) -> Result<[Mutation; 2], AppError> {
    let count_key = keys::shard_count(table_id, region);
    let count = match ShardEntry::leaf_count(entry)? {
        Some(count) => put_mutation(count_key, count.to_le_bytes().to_vec()),
        // ポインタノードは件数を持たない。リーフから昇格した場合に備えて消す。
        None => delete_mutation(count_key),
    };
    Ok([
        put_mutation(keys::shard(table_id, region), frame(entry)),
        count,
    ])
}

pub(super) fn shard_deletions(table_id: TableId, region: &FlexId) -> [Mutation; 2] {
    [
        delete_mutation(keys::shard(table_id, region)),
        delete_mutation(keys::shard_count(table_id, region)),
    ]
}

/// 変更前後で同じキーは触らない（消してから書き直すと MVCC のバージョンが無駄に増える）。
pub(super) fn value_index_mutations(
    old_keys: &FxHashSet<Vec<u8>>,
    new_keys: &FxHashSet<Vec<u8>>,
    out: &mut Vec<Mutation>,
) {
    out.extend(old_keys.difference(new_keys).cloned().map(delete_mutation));
    out.extend(
        new_keys
            .difference(old_keys)
            .map(|key| put_mutation(key.clone(), Vec::new())),
    );
}

/// シャード領域をまとめてロックし、**ロック時点の**内容を返す（未作成領域は `None`）。
///
/// データ経路はシャードのキー単位で完結するので、テーブル全体は排他しない。別リーフへの
/// 書き込みは別インスタンスからでも並列に流れる。存在しないキーもロックされるため、
/// 「空の領域を他者が埋める」「親へ畳んで消す」も排他できる。
///
/// キーを [`BTreeMap`] に集めるのは、反復順がそのままデッドロック回避の全順序になるため。
/// **このトランザクションで既に書いた内容が重なった状態**で返る。
pub(super) async fn lock_shards(
    txn: &Readers<LazyTxn>,
    table_id: TableId,
    regions: impl IntoIterator<Item = FlexId>,
) -> Result<BTreeMap<FlexId, Option<ShardValue>>, AppError> {
    let by_key: BTreeMap<Vec<u8>, FlexId> = regions
        .into_iter()
        .map(|r| (keys::shard(table_id, &r), r))
        .collect();
    if by_key.is_empty() {
        return Ok(BTreeMap::new());
    }

    let mut found = {
        let mut txn = txn.lock().await;
        txn.lock_and_read(by_key.keys().cloned().collect()).await?
    };

    let mut out = BTreeMap::new();
    for (key, region) in by_key {
        let value = match found.remove(&key) {
            Some(framed) => Some(ShardValue::try_from(framed)?),
            None => None,
        };
        out.insert(region, value);
    }
    Ok(out)
}

/// プレフィックスに一致するキーを上限件数まで削除し、削除した件数を返す（0 なら消し切り）。
///
/// 1 トランザクションに全部を詰めると TiKV のサイズ上限に当たるので、繰り返し呼べる形にする。
pub(super) async fn delete_prefix_chunk(
    txn: &Readers<LazyTxn>,
    prefix: &[u8],
    limit: u32,
) -> Result<usize, AppError> {
    let end = keys::prefix_end(prefix);
    let batch = {
        let mut txn = txn.lock().await;
        let raw = txn
            .read_range_keys(range_from(prefix.to_vec(), end.as_deref()), limit)
            .await?;
        // 既に消したキーを数えると「まだ残っている」と読めて繰り返しが止まらなくなる。
        let staged = txn.staged_range(prefix, end.as_deref());
        if staged.is_empty() {
            raw
        } else {
            let deleted: BTreeSet<&Vec<u8>> = staged
                .iter()
                .filter(|(_, value)| value.is_none())
                .map(|(key, _)| key)
                .collect();
            raw.into_iter()
                .filter(|key| !deleted.contains(key))
                .collect()
        }
    };
    let removed = batch.len();
    delete_many(txn, batch).await;
    Ok(removed)
}

/// 件数キーだけを読んで合算する。
pub(super) async fn table_flex_id_count<R: Reader>(
    txn: &Readers<R>,
    table_id: TableId,
) -> Result<u64, AppError> {
    let entries = scan_prefix(txn, &keys::shard_counts_of(table_id)).await?;
    let mut total = 0u64;
    for (_, value) in entries {
        let bytes: [u8; 4] = value.as_slice().try_into().map_err(|_| {
            AppError::StorageError("shard count entry is not four bytes".to_string())
        })?;
        total += u32::from_le_bytes(bytes) as u64;
    }
    Ok(total)
}

pub(super) async fn batch_get_shards<R: Reader>(
    txn: &Readers<R>,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, ShardValue)>, AppError> {
    batch_get(txn, keys)
        .await?
        .into_iter()
        .map(|(key, framed)| Ok((key, ShardValue::try_from(framed)?)))
        .collect()
}

pub(super) async fn scan_shard_prefix<R: Reader>(
    txn: &Readers<R>,
    prefix: &[u8],
) -> Result<Vec<(Vec<u8>, ShardValue)>, AppError> {
    scan_prefix(txn, prefix)
        .await?
        .into_iter()
        .map(|(key, framed)| Ok((key, ShardValue::try_from(framed)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_value_round_trips() {
        let entry = b"shard entry bytes".to_vec();
        let value = ShardValue::try_from(frame(&entry)).unwrap();
        assert_eq!(value.entry(), entry.as_slice());
    }

    #[test]
    fn empty_entry_round_trips() {
        let value = ShardValue::try_from(frame(&[])).unwrap();
        assert_eq!(value.entry(), b"");
    }

    #[test]
    fn corrupted_payload_is_rejected() {
        let mut framed = frame(b"shard entry bytes");
        // ペイロードの 1 バイトだけを壊す（CRC ヘッダは触らない）。
        let last = framed.len() - 1;
        framed[last] ^= 0xFF;
        assert!(ShardValue::try_from(framed).is_err());
    }

    #[test]
    fn truncated_value_is_rejected() {
        let mut framed = frame(b"shard entry bytes");
        framed.truncate(framed.len() - 3);
        assert!(ShardValue::try_from(framed).is_err());
        // ヘッダにも届かない長さも弾く（スライスで panic しないこと）。
        assert!(ShardValue::try_from(vec![0x00, 0x01]).is_err());
        assert!(ShardValue::try_from(Vec::new()).is_err());
    }
}
