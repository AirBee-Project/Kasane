//! トランザクション上の素の KV 操作と、シャード値のフレーム。
//!
//! 読み取りは [`TikvRead`](super::TikvRead) と [`TikvWrite`](super::TikvWrite) の
//! どちらからも必要なので、トランザクションだけを受け取る自由関数として置いている。
//!
//! # なぜ読み取りが [`Reader`] で抽象化されているか
//!
//! 通常の読み書きは [`tikv_client::Transaction`] 上で行うが、クエリ実行器の入力源
//! （`query_source.rs`）は**開始タイムスタンプを固定した**読み取りを何度も行う必要があり、
//! そちらは [`tikv_client::Snapshot`] を使う。両者は読み取り系のシグネチャが同一なので、
//! ここで 1 つの trait にまとめ、下の自由関数を両方で使い回す。

use std::sync::Arc;

use kasane_logic::FlexId;
use tikv_client::{BoundRange, Snapshot, Transaction, TransactionClient};
use tokio::sync::Mutex;

use super::keys;
use crate::error::AppError;
use crate::models::id::TableId;
use crate::repositories::encoding::shard_entry::ShardEntry;

use super::to_app_error;

/// 1 回のスキャンで取り出す件数。TiKV は 1 リクエストの上限があるため分割して読む。
const SCAN_BATCH: u32 = 512;

// --- 読み取りの抽象 ---

/// スナップショット読み取りができるもの（[`Transaction`] と [`Snapshot`]）。
///
/// 下の自由関数がこの trait 越しに動くので、呼び出し側は「どちらのスナップショットか」
/// を意識せずに同じ読み取り経路を通れる。
///
/// `pub` なのは [`TikvRead`](super::TikvRead) の型引数の境界に現れるためだけで、
/// バックエンドの内部事情。`kv` モジュール自体が非公開なので外からは名指しできない。
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
}

/// [`Transaction`] と [`Snapshot`] は読み取り系のシグネチャが完全に一致するので、
/// 実装は 1 つの雛形から生成する。
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

// --- 作業トランザクション（遅延生成） ---

/// 書き込み側のトランザクション。**最初に実際へ触れるまで開かない。**
///
/// 書き込みクロージャの 1 周目は、必要なロックを宣言した時点で巻き戻される
/// （`super::mod` の「必要なロックをどう知るか」）。その周回でトランザクションを
/// 開いてしまうと、何もせずに捨てるためだけに PD からタイムスタンプを取ることになる。
/// 遅延生成にすると、1 周目はネットワークに一切触れずに終わる。
///
/// 開くのが遅れることは正しさを損なわない。むしろ「ロックを取ってから
/// `start_ts` を確定させる」という不変条件を、より遅い側へ寄せて強めている。
pub struct LazyTxn {
    client: Arc<TransactionClient>,
    txn: Option<Transaction>,
}

impl LazyTxn {
    pub(super) fn new(client: Arc<TransactionClient>) -> Self {
        Self { client, txn: None }
    }

    async fn open(&mut self) -> Result<&mut Transaction, tikv_client::Error> {
        if self.txn.is_none() {
            self.txn = Some(self.client.begin_pessimistic().await?);
        }
        Ok(self.txn.as_mut().expect("直前に開いた"))
    }

    /// 開いていたトランザクションを取り出す。一度も触れていなければ `None`。
    pub(super) fn into_opened(self) -> Option<Transaction> {
        self.txn
    }

    async fn put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), tikv_client::Error> {
        self.open().await?.put(key, value).await
    }

    async fn delete(&mut self, key: Vec<u8>) -> Result<(), tikv_client::Error> {
        self.open().await?.delete(key).await
    }
}

impl Reader for LazyTxn {
    async fn read_one(&mut self, key: Vec<u8>) -> Result<Option<Vec<u8>>, tikv_client::Error> {
        self.open().await?.read_one(key).await
    }

    async fn read_many(
        &mut self,
        keys: Vec<Vec<u8>>,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error> {
        self.open().await?.read_many(keys).await
    }

    async fn read_range(
        &mut self,
        range: BoundRange,
        limit: u32,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error> {
        self.open().await?.read_range(range, limit).await
    }

    async fn read_range_keys(
        &mut self,
        range: BoundRange,
        limit: u32,
    ) -> Result<Vec<Vec<u8>>, tikv_client::Error> {
        self.open().await?.read_range_keys(range, limit).await
    }
}

// --- 素の KV 操作 ---

pub(super) async fn get<R: Reader>(
    txn: &Mutex<R>,
    key: Vec<u8>,
) -> Result<Option<Vec<u8>>, AppError> {
    let mut txn = txn.lock().await;
    txn.read_one(key).await.map_err(to_app_error)
}

pub(super) async fn put(
    txn: &Mutex<LazyTxn>,
    key: Vec<u8>,
    value: Vec<u8>,
) -> Result<(), AppError> {
    let mut txn = txn.lock().await;
    txn.put(key, value).await.map_err(to_app_error)
}

pub(super) async fn delete(txn: &Mutex<LazyTxn>, key: Vec<u8>) -> Result<(), AppError> {
    let mut txn = txn.lock().await;
    txn.delete(key).await.map_err(to_app_error)
}

/// 複数のキーをまとめて書く。
///
/// `put` を 1 件ずつ呼ぶとキーの数だけミューテックスを取り直すことになる。
/// tikv-client 側では書き込みはローカルバッファへの追記なので、
/// ロックを 1 回にまとめれば往復もロック競合も減る。
pub(super) async fn put_many(
    txn: &Mutex<LazyTxn>,
    entries: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
) -> Result<(), AppError> {
    let mut txn = txn.lock().await;
    for (key, value) in entries {
        txn.put(key, value).await.map_err(to_app_error)?;
    }
    Ok(())
}

/// 複数のキーをまとめて消す（[`put_many`] と同じ理由）。
pub(super) async fn delete_many(
    txn: &Mutex<LazyTxn>,
    keys: impl IntoIterator<Item = Vec<u8>>,
) -> Result<(), AppError> {
    let mut txn = txn.lock().await;
    for key in keys {
        txn.delete(key).await.map_err(to_app_error)?;
    }
    Ok(())
}

/// 複数キーをまとめて引く。存在しないキーは結果に現れない。
pub(super) async fn batch_get<R: Reader>(
    txn: &Mutex<R>,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let mut txn = txn.lock().await;
    txn.read_many(keys).await.map_err(to_app_error)
}

fn range_from(start: Vec<u8>, end: Option<Vec<u8>>) -> BoundRange {
    match end {
        Some(end) => BoundRange::from(start..end),
        None => BoundRange::from(start..),
    }
}

/// プレフィックスに一致する全キーと値を取り出す（バッチ分割して読み切る）。
pub(super) async fn scan_prefix<R: Reader>(
    txn: &Mutex<R>,
    prefix: &[u8],
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    let end = keys::prefix_end(prefix);
    let mut start = prefix.to_vec();
    let mut out: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();

    loop {
        let batch = {
            let mut txn = txn.lock().await;
            txn.read_range(range_from(start.clone(), end.clone()), SCAN_BATCH)
                .await
                .map_err(to_app_error)?
        };

        let fetched = batch.len();
        out.extend(batch);
        if fetched < SCAN_BATCH as usize {
            break;
        }
        // 次のバッチは最後に読んだキーの直後から。
        start = out.last().expect("バッチが空でない").0.clone();
        start.push(0);
    }

    Ok(out)
}

/// 指定範囲のキーだけを取り出す（バッチ分割して読み切る）。
async fn scan_keys_range<R: Reader>(
    txn: &Mutex<R>,
    start: Vec<u8>,
    end: Option<Vec<u8>>,
) -> Result<Vec<Vec<u8>>, AppError> {
    let mut cursor = start;
    let mut out: Vec<Vec<u8>> = Vec::new();

    loop {
        let batch = {
            let mut txn = txn.lock().await;
            txn.read_range_keys(range_from(cursor.clone(), end.clone()), SCAN_BATCH)
                .await
                .map_err(to_app_error)?
        };

        let fetched = batch.len();
        out.extend(batch);
        if fetched < SCAN_BATCH as usize {
            break;
        }
        // 次のバッチは最後に読んだキーの直後から。
        cursor = out.last().expect("バッチが空でない").clone();
        cursor.push(0);
    }

    Ok(out)
}

/// プレフィックスに一致する全キーだけを取り出す。
pub(super) async fn scan_prefix_keys<R: Reader>(
    txn: &Mutex<R>,
    prefix: &[u8],
) -> Result<Vec<Vec<u8>>, AppError> {
    scan_keys_range(txn, prefix.to_vec(), keys::prefix_end(prefix)).await
}

/// `start`（含む）から `end_inclusive`（含む）までのキーを取り出す。
pub(super) async fn scan_inclusive_keys<R: Reader>(
    txn: &Mutex<R>,
    start: Vec<u8>,
    end_inclusive: Vec<u8>,
) -> Result<Vec<Vec<u8>>, AppError> {
    // 終端を含めたいので、末尾に 0 を足して排他終端へ変換する。
    let mut end = end_inclusive;
    end.push(0);
    scan_keys_range(txn, start, Some(end)).await
}

// --- シャード値のフレーム ---
//
// シャードのペイロードは `SpatialIdMap` の rkyv バイト列で、読み出しは
// `access_unchecked`（構造を検証しないゼロコピーアクセス）を通る。その安全条件は
// 「自分が書いたバイト列そのものであること」で、ローカルの mmap ならファイル権限が
// それを担保していた。TiKV ではバイト列がネットワーク越しに届き、クラスタは
// 複数インスタンス・複数バージョンで共有されうるため、前提が成り立たない。
//
// そこで TiKV に保存するシャード値だけを CRC32 で包み、`unsafe` へ渡す前に検証する。
// 検証はペイロードを 1 度なめるだけで、コピーは発生しない（[`ShardValue::entry`] は
// 受信バッファへの借用を返す）ので、ゼロコピーはそのまま保たれる。
//
// LMDB 側は共通形式（`encoding::shard_entry`）のまま。両バックエンドで同じなのは
// **ペイロードの形式**であり、この枠はその外側の TiKV 固有の保存表現にあたる。

/// フレームのヘッダ長 = CRC32（u32 LE, 4）。
const FRAME_HEADER_LEN: usize = 4;

/// CRC 検証を通ったシャード値。
///
/// 受信したバッファをそのまま保持し、[`entry`](Self::entry) が検証済みペイロードへの
/// 借用を返す。ペイロードを別バッファへ写さないので、この先の rkyv アクセスは
/// 受信バッファを直接読む。
pub(super) struct ShardValue {
    framed: Vec<u8>,
}

impl ShardValue {
    /// 保存されていたバイト列を検証して受け取る。
    ///
    /// CRC が合わない場合は「壊れている」ものとして拒否する。ここを通過したバイト列だけが
    /// `access_unchecked` へ渡る。
    fn verify(framed: Vec<u8>) -> Result<Self, AppError> {
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
                "shard value failed its integrity check (crc32 expected {expected:#010x}, found {actual:#010x})"
            )));
        }
        Ok(Self { framed })
    }

    /// 検証済みのシャードエントリ（`encoding::shard_entry` の形式）。
    pub(super) fn entry(&self) -> &[u8] {
        &self.framed[FRAME_HEADER_LEN..]
    }
}

/// シャードエントリを保存用のフレームへ包む。
fn frame(entry: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME_HEADER_LEN + entry.len());
    out.extend_from_slice(&crc32fast::hash(entry).to_le_bytes());
    out.extend_from_slice(entry);
    out
}

// --- シャードの保存 ---
//
// シャードは 2 本のキーで表される。
//
// ```text
//   0x06 ‖ table_id ‖ flex_id -> crc32 ‖ ShardEntry     （本体）
//   0x08 ‖ table_id ‖ flex_id -> 保持件数（u32 LE）      （リーフのみ）
// ```
//
// 件数はエントリのヘッダにも入っているが、そちらを読むにはリーフの本体
// （`SpatialIdMap` の rkyv バイト列）ごと転送することになる。`table_count` は
// 件数を合算するだけなので、テーブル全体を転送するのは割に合わない。件数だけを
// 別キーへ切り出すと、集計はシャード数 × 4 バイトで済む。
//
// LMDB 側にこのキーは無い。あちらは mmap 上をストリームで舐められるので、
// 本体を持ってくる代償が無いため。`keys.rs` はもともとバックエンド固有なので、
// この差はレイアウトの差として閉じている。
//
// 2 本のキーが食い違わないよう、書き込みと削除は必ずこの節の関数を通す。

/// シャードエントリを保存する。リーフなら件数キーも同時に更新する。
pub(super) async fn put_shard(
    txn: &Mutex<LazyTxn>,
    table_id: TableId,
    region: &FlexId,
    entry: &[u8],
) -> Result<(), AppError> {
    let count_key = keys::shard_count(table_id, region);
    let mut txn_guard = txn.lock().await;
    txn_guard
        .put(keys::shard(table_id, region), frame(entry))
        .await
        .map_err(to_app_error)?;
    match ShardEntry::leaf_count(entry)? {
        Some(count) => txn_guard
            .put(count_key, count.to_le_bytes().to_vec())
            .await
            .map_err(to_app_error),
        // ポインタノードは件数を持たない。リーフから昇格した場合に備えて消す。
        None => txn_guard.delete(count_key).await.map_err(to_app_error),
    }
}

/// シャードエントリを削除する（件数キーも一緒に消える）。
pub(super) async fn delete_shard(
    txn: &Mutex<LazyTxn>,
    table_id: TableId,
    region: &FlexId,
) -> Result<(), AppError> {
    let mut txn_guard = txn.lock().await;
    txn_guard
        .delete(keys::shard(table_id, region))
        .await
        .map_err(to_app_error)?;
    txn_guard
        .delete(keys::shard_count(table_id, region))
        .await
        .map_err(to_app_error)
}

/// テーブルが保持する [`FlexId`] の総数を、件数キーだけを読んで合算する。
pub(super) async fn table_flex_id_count<R: Reader>(
    txn: &Mutex<R>,
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

pub(super) async fn get_shard<R: Reader>(
    txn: &Mutex<R>,
    key: Vec<u8>,
) -> Result<Option<ShardValue>, AppError> {
    match get(txn, key).await? {
        Some(framed) => Ok(Some(ShardValue::verify(framed)?)),
        None => Ok(None),
    }
}

pub(super) async fn batch_get_shards<R: Reader>(
    txn: &Mutex<R>,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, ShardValue)>, AppError> {
    batch_get(txn, keys)
        .await?
        .into_iter()
        .map(|(key, framed)| Ok((key, ShardValue::verify(framed)?)))
        .collect()
}

pub(super) async fn scan_shard_prefix<R: Reader>(
    txn: &Mutex<R>,
    prefix: &[u8],
) -> Result<Vec<(Vec<u8>, ShardValue)>, AppError> {
    scan_prefix(txn, prefix)
        .await?
        .into_iter()
        .map(|(key, framed)| Ok((key, ShardValue::verify(framed)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_value_round_trips() {
        let entry = b"shard entry bytes".to_vec();
        let value = ShardValue::verify(frame(&entry)).unwrap();
        assert_eq!(value.entry(), entry.as_slice());
    }

    #[test]
    fn empty_entry_round_trips() {
        let value = ShardValue::verify(frame(&[])).unwrap();
        assert_eq!(value.entry(), b"");
    }

    #[test]
    fn corrupted_payload_is_rejected() {
        let mut framed = frame(b"shard entry bytes");
        // ペイロードの 1 バイトだけを壊す（CRC ヘッダは触らない）。
        let last = framed.len() - 1;
        framed[last] ^= 0xFF;
        assert!(ShardValue::verify(framed).is_err());
    }

    #[test]
    fn truncated_value_is_rejected() {
        let mut framed = frame(b"shard entry bytes");
        framed.truncate(framed.len() - 3);
        assert!(ShardValue::verify(framed).is_err());
        // ヘッダにも届かない長さも弾く（スライスで panic しないこと）。
        assert!(ShardValue::verify(vec![0x00, 0x01]).is_err());
        assert!(ShardValue::verify(Vec::new()).is_err());
    }
}
