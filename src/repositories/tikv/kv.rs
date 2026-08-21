//! トランザクション上の素の KV 操作と、シャード値のフレーム。
//!
//! 読み書きどちらからも必要なので、読み取り元だけを受け取る自由関数にしてある。

#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use kasane_logic::FlexId;
use rustc_hash::FxHashSet;
pub(super) use tikv_client::transaction::Mutation;
use tikv_client::{BoundRange, Snapshot, Timestamp, Transaction, TransactionClient};
use tokio::sync::{Mutex, MutexGuard};

use super::keys;
use crate::error::{AppError, Stored};
use crate::models::id::TableId;
use crate::repositories::encoding::shard_entry::{MAX_SHARD_BYTES, ShardEntry};

/// 1 回のスキャンで取り出す件数。TiKV は 1 リクエストの上限があるため分割して読む。
const SCAN_BATCH: u32 = 512;

/// 1 リクエストへ載せるキー数の上限。
///
/// 同じテーブルのシャードキーは 1 リージョンに集まりやすく、そのまま渡すと gRPC の
/// メッセージサイズ上限に触れる。分割してもキーの昇順は保たれる。
///
/// カタログ・ACL など小さな値専用。シャード値は [`SHARD_BATCH_KEYS`] を使う。
const BATCH_KEYS: usize = 1024;

/// シャード値専用の 1 リクエストあたり件数上限。
///
/// シャードの葉は 1 件で最大 [`MAX_SHARD_BYTES`] ある。[`BATCH_KEYS`] のまま使うと
/// 1 RPC が数百 MB に膨らみ、gRPC のメッセージサイズ上限（受信側でも設定される）に
/// 触れうる。1 RPC あたりのバイト量を [`SHARD_BATCH_BYTE_BUDGET`] 以下に抑える件数へ
/// 落とす。
const SHARD_BATCH_BYTE_BUDGET: usize = 4 * 1024 * 1024;
const SHARD_BATCH_KEYS: usize = SHARD_BATCH_BYTE_BUDGET / MAX_SHARD_BYTES;

/// 1 チャンクが既にリージョン単位で内部並行化されているので、ここを大きくしても
/// 増えるのは同時に飛ぶリクエスト数と手元に載る応答の量だけ。
const MAX_FANOUT: usize = 8;

/// スナップショット読み取りができるもの（[`Transaction`] / [`Snapshot`] / [`LazyTxn`]）。
///
/// `pub` なのは [`TikvRead`](super::TikvRead) の境界に現れるためだけ。`kv` は非公開。
// `async fn` の Future に `Send` を課さない理由は `traits::storage` を参照。
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

    /// `start`（含む）〜`end`（排他）の**まだ送っていない変更**をキー昇順で返す
    /// （`None` は削除）。溜める仕組みを持たない読み取り元では常に空。
    ///
    /// 範囲読みが自分の変更をその場で重ねられないのは、読み切る関数が取得件数を終端の判定に
    /// 使うため。途中で件数を増減させると読み切る前に止まる。
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
/// [`Reader`] は `&mut self` を要求するので 1 つの実体では逐次にしか読めない。断面が ts で
/// 決まる読み取りだけ追加で開ける（書き込み側は**未送信の変更**を重ねる必要があるので不可）。
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
/// ロックを宣言しただけで巻き戻る 1 周目が、捨てるためだけに PD から ts を取るのを避ける。
pub struct LazyTxn {
    client: Arc<TransactionClient>,
    txn: Option<Transaction>,
    one_pc: bool,
    /// まだ TiKV へ送っていない変更（`None` は削除）。コミット直前に 1 回だけ流す。
    ///
    /// 都度 `batch_mutate` を投げると触れたリーフの数だけリクエストが直列に積み上がる。
    /// [`BTreeMap`] なのは、キー昇順で送るとリージョン跨ぎが減り重複も畳まれるため。
    /// **読み取りに自分の書き込みを見せる**用途も兼ねる。
    pending: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    /// ロックの取得はこの**内側**で起きるので、競合は `Storage::write` からは
    /// 「クロージャが返したエラー」に見える。判断できるよう記録する。
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

    /// **このトランザクションが出す失敗は読み書きを問わずここを通すこと。** 通し忘れると
    /// やり直されずに 500 になる。木の降下も他者のロックに当たるので読み取り側も対象。
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
        Ok(self.txn.as_mut().expect("just opened above"))
    }

    /// 取り出した後は [`Drop`] の対象が無くなるので、後始末は呼び出し側の責任になる。
    pub(super) fn into_opened(mut self) -> Option<Transaction> {
        self.txn.take()
    }

    fn stage(&mut self, mutations: impl IntoIterator<Item = Mutation>) {
        for m in mutations {
            match m {
                Mutation::Put(key, value) => {
                    self.pending.insert(key.into(), Some(value));
                }
                Mutation::Delete(key) => {
                    self.pending.insert(key.into(), None);
                }
            }
        }
    }

    /// 溜めた変更をまとめて送り、悲観ロックを取る。**コミットの直前に 1 度だけ呼ぶ。**
    ///
    /// 捨てる試行では呼ばない。溜めたまま drop すればロックも MVCC の版も作られない。
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
    /// 「ロックしてから `get`」だと `get` が `start_ts` を読むので、その間のコミットを見落とす。
    /// 存在しないキーも**ロックされる**（空領域を他者が埋めるのを防ぐのに使っている）。
    /// 応答に自分の未送信の変更は映らないので、手元の台帳を優先させる。
    async fn lock_and_read(
        &mut self,
        mut keys: Vec<Vec<u8>>,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, tikv_client::Error> {
        // キーは昇順で渡ってくる。分割しても順序は保たれる。
        let mut locked: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        let all_keys = keys.clone();
        // 常にシャードキーだけを受け取る（唯一の呼び出し元は `lock_shards`）ので、
        // カタログ用の `BATCH_KEYS` ではなく `SHARD_BATCH_KEYS` で刻む。
        while !keys.is_empty() {
            let rest = keys.split_off(keys.len().min(SHARD_BATCH_KEYS));
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

/// 読み取りは**未送信の変更を先に見る**（「書いてから読む」を成立させるため）。そして
/// **失敗は必ず [`note`](LazyTxn::note) を通す**（競合の取りこぼしを防ぐため）。
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
    Mutation::Put(key.into(), value)
}

pub(super) fn delete_mutation(key: Vec<u8>) -> Mutation {
    Mutation::Delete(key.into())
}

/// 存在しないキーは結果に現れない。
///
/// チャンクは互いに独立なので、追加スナップショットを開ける読み取り元では並行に投げる。
pub(super) async fn batch_get<R: Reader>(
    txn: &Readers<R>,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    batch_get_chunked(txn, keys, BATCH_KEYS).await
}

/// シャード値専用。値が大きいので [`SHARD_BATCH_KEYS`] でより細かく刻む。
pub(super) async fn batch_get_shards<R: Reader>(
    txn: &Readers<R>,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, ShardValue)>, AppError> {
    batch_get_chunked(txn, keys, SHARD_BATCH_KEYS)
        .await?
        .into_iter()
        .map(|(key, framed)| Ok((key, ShardValue::try_from(framed)?)))
        .collect()
}

async fn batch_get_chunked<R: Reader>(
    txn: &Readers<R>,
    keys: Vec<Vec<u8>>,
    chunk_size: usize,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    if keys.len() > chunk_size
        && let Some(fanout) = &txn.fanout
    {
        return batch_get_fanned_out(fanout, keys, chunk_size).await;
    }

    let mut txn = txn.lock().await;
    // 収まる場合はそのまま渡す（`chunks(..).to_vec()` だとキー列を丸ごと複製する）。
    if keys.len() <= chunk_size {
        return txn.read_many(keys).await.map_err(AppError::from);
    }

    let mut rest = keys;
    let mut out = Vec::new();
    while !rest.is_empty() {
        let tail = rest.split_off(rest.len().min(chunk_size));
        out.extend(txn.read_many(rest).await?);
        rest = tail;
    }
    Ok(out)
}

/// チャンクごとに独立したスナップショットを開き、[`MAX_FANOUT`] 本まで並行に読む。
///
/// **失敗しても飛んでいる RPC を打ち切らない。** [`JoinSet`](tokio::task::JoinSet) は drop で
/// 中のタスクを abort するので、1 本の失敗で `?` を返すと兄弟が RST で切られる。混んでいる
/// ときほど増えるので、詰まりかけたクラスタへこちらからキャンセルを浴びせる形になる。
async fn batch_get_fanned_out(
    fanout: &Fanout,
    keys: Vec<Vec<u8>>,
    chunk_size: usize,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    type ChunkResult = Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error>;

    let mut chunks = keys.chunks(chunk_size).map(<[Vec<u8>]>::to_vec);
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
/// **判定は生の取得件数で行う。** 未送信の変更を重ねるのは読み切ったあと。`batch` は
/// そのスキャンで実際に指定した 1 リクエストあたりの上限（呼び出し元ごとに異なる）。
fn next_cursor(last_key: &[u8], fetched: usize, batch: usize) -> Option<Vec<u8>> {
    if fetched < batch {
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
    scan_prefix_with_batch(txn, prefix, SCAN_BATCH).await
}

/// `batch` はシャード値なら [`SHARD_BATCH_KEYS`]、それ以外は [`SCAN_BATCH`]。
async fn scan_prefix_with_batch<R: Reader>(
    txn: &Readers<R>,
    prefix: &[u8],
    batch: u32,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    let end = keys::prefix_end(prefix);
    let mut cursor = Some(prefix.to_vec());
    let mut out: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();

    while let Some(start) = cursor {
        let fetched_batch = {
            let mut txn = txn.lock().await;
            txn.read_range(range_from(start, end.as_deref()), batch)
                .await?
        };
        let fetched = fetched_batch.len();
        out.extend(fetched_batch);
        cursor = out
            .last()
            .and_then(|(key, _)| next_cursor(key, fetched, batch as usize));
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
        cursor = out
            .last()
            .and_then(|key| next_cursor(key, fetched, SCAN_BATCH as usize));
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

/// この前置に一致する行が 1 つでもあるか。
///
/// 「配下のどれかに届くか」を知るだけの判定で、全件を読まないために使う。
pub(super) async fn any_key_in_prefix<R: Reader>(
    txn: &Readers<R>,
    prefix: &[u8],
) -> Result<bool, AppError> {
    let end = keys::prefix_end(prefix);

    // 手元で消したぶんは実在しない。消した数だけ余分に読めば、重ねたあとに残るかが判る。
    let staged = txn.lock().await.staged_range(prefix, end.as_deref());
    if staged.iter().any(|(_, value)| value.is_some()) {
        return Ok(true);
    }
    let deleted: BTreeSet<&Vec<u8>> = staged.iter().map(|(key, _)| key).collect();

    let budget = deleted.len().saturating_add(1).min(u32::MAX as usize) as u32;
    let raw = {
        let mut txn = txn.lock().await;
        txn.read_range_keys(range_from(prefix.to_vec(), end.as_deref()), budget)
            .await?
    };
    Ok(raw.iter().any(|key| !deleted.contains(key)))
}

/// `start`（含む）〜`end`（排他）を最大 `limit` 件。ページングに使う。
pub(super) async fn scan_range_limited<R: Reader>(
    txn: &Readers<R>,
    start: Vec<u8>,
    end: Option<Vec<u8>>,
    limit: usize,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    // 読み切らないので「全部読んでから重ねる」ができない。手元で消したぶんだけ余分に
    // 読んでおけば、重ねたあとに `limit` 件を満たせる。
    let staged = txn.lock().await.staged_range(&start, end.as_deref());
    let deleted = staged.iter().filter(|(_, value)| value.is_none()).count();
    let budget = limit.saturating_add(deleted).min(u32::MAX as usize) as u32;

    let raw = {
        let mut txn = txn.lock().await;
        txn.read_range(range_from(start, end.as_deref()), budget)
            .await?
    };

    if staged.is_empty() {
        return Ok(raw);
    }
    let mut merged: BTreeMap<Vec<u8>, Vec<u8>> = raw.into_iter().collect();
    for (key, value) in staged {
        match value {
            Some(value) => merged.insert(key, value),
            None => merged.remove(&key),
        };
    }
    Ok(merged.into_iter().take(limit).collect())
}

/// CRC32（u32 LE）。
const FRAME_HEADER_LEN: usize = 4;

/// CRC 検証を通ったシャード値。LMDB 側にこの枠は無い。
///
/// CRC32 で包むのは、読み出しが `access_unchecked`（構造を検証しないゼロコピーアクセス）を
/// 通るため。その安全条件「自分が書いたバイト列そのもの」は、ネットワーク越しに届き複数
/// インスタンスで共有されうる TiKV では成り立たない。検証にコピーは発生しない。
///
/// `Arc<[u8]>` で持つのは `Clone` を安く保つため。クエリ用のノードキャッシュ
/// （[`tree::NodeCache`](super::tree::NodeCache)）はこれを複製して溜めるので、シャード
/// 本体（数 MB になりうる）を毎回複製すると意味が無い。
#[derive(Clone)]
pub(super) struct ShardValue {
    framed: Arc<[u8]>,
}

/// ここを通過したバイト列だけが `access_unchecked` へ渡る。
impl TryFrom<Vec<u8>> for ShardValue {
    type Error = AppError;

    fn try_from(framed: Vec<u8>) -> Result<Self, AppError> {
        if framed.len() < FRAME_HEADER_LEN {
            return Err(AppError::corrupt(
                Stored::Shard,
                "value is shorter than its frame header",
            ));
        }
        let expected = u32::from_le_bytes(
            framed[..FRAME_HEADER_LEN]
                .try_into()
                .expect("at least FRAME_HEADER_LEN bytes"),
        );
        let actual = crc32fast::hash(&framed[FRAME_HEADER_LEN..]);
        if actual != expected {
            return Err(AppError::StorageError(format!(
                "shard value failed its integrity check \
                 (crc32 expected {expected:#010x}, found {actual:#010x})"
            )));
        }
        Ok(Self {
            framed: Arc::from(framed),
        })
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
/// 件数を別キー（`0x08`）へ切り出すのは、ヘッダから読むとリーフ本体ごと転送することになり
/// `table_count` に割に合わないため（LMDB 側にこのキーは無い）。2 本を必ず一組で返すのが、
/// 片方だけ書いて食い違わせないための入口。
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
/// キー単位で完結するのでテーブル全体は排他しない。[`BTreeMap`] に集めるのは、反復順が
/// デッドロック回避の全順序になるため。**既に書いた内容が重なった状態**で返る。
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
        let bytes: [u8; 4] = value
            .as_slice()
            .try_into()
            .map_err(|_| AppError::corrupt(Stored::Shard, "count entry is not four bytes"))?;
        total += u32::from_le_bytes(bytes) as u64;
    }
    Ok(total)
}

pub(super) async fn scan_shard_prefix<R: Reader>(
    txn: &Readers<R>,
    prefix: &[u8],
) -> Result<Vec<(Vec<u8>, ShardValue)>, AppError> {
    scan_prefix_with_batch(txn, prefix, SHARD_BATCH_KEYS as u32)
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
