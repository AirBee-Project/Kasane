//! トランザクション上の素の KV 操作。
//!
//! 読み取りは [`TikvRead`](super::TikvRead) と [`TikvWrite`](super::TikvWrite) の
//! どちらからも必要なので、トランザクションだけを受け取る自由関数として置いている。

use tikv_client::{BoundRange, Transaction};
use tokio::sync::Mutex;

use super::keys;
use crate::error::AppError;

use super::to_app_error;

/// 1 回のスキャンで取り出す件数。TiKV は 1 リクエストの上限があるため分割して読む。
const SCAN_BATCH: u32 = 512;

pub(super) async fn get(
    txn: &Mutex<Transaction>,
    key: Vec<u8>,
) -> Result<Option<Vec<u8>>, AppError> {
    let mut txn = txn.lock().await;
    txn.get(key).await.map_err(to_app_error)
}

pub(super) async fn put(
    txn: &Mutex<Transaction>,
    key: Vec<u8>,
    value: Vec<u8>,
) -> Result<(), AppError> {
    let mut txn = txn.lock().await;
    txn.put(key, value).await.map_err(to_app_error)
}

pub(super) async fn delete(txn: &Mutex<Transaction>, key: Vec<u8>) -> Result<(), AppError> {
    let mut txn = txn.lock().await;
    txn.delete(key).await.map_err(to_app_error)
}

/// 複数キーをまとめて引く。存在しないキーは結果に現れない。
pub(super) async fn batch_get(
    txn: &Mutex<Transaction>,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let mut txn = txn.lock().await;
    let pairs = txn.batch_get(keys).await.map_err(to_app_error)?;
    Ok(pairs.map(|kv| (Vec::from(kv.0), kv.1)).collect())
}

fn range_from(start: Vec<u8>, end: Option<Vec<u8>>) -> BoundRange {
    match end {
        Some(end) => BoundRange::from(start..end),
        None => BoundRange::from(start..),
    }
}

/// プレフィックスに一致する全キーと値を取り出す（バッチ分割して読み切る）。
pub(super) async fn scan_prefix(
    txn: &Mutex<Transaction>,
    prefix: &[u8],
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    let end = keys::prefix_end(prefix);
    let mut start = prefix.to_vec();
    let mut out: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();

    loop {
        let batch: Vec<(Vec<u8>, Vec<u8>)> = {
            let mut txn = txn.lock().await;
            txn.scan(range_from(start.clone(), end.clone()), SCAN_BATCH)
                .await
                .map_err(to_app_error)?
                .map(|kv| (Vec::from(kv.0), kv.1))
                .collect()
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
async fn scan_keys_range(
    txn: &Mutex<Transaction>,
    start: Vec<u8>,
    end: Option<Vec<u8>>,
) -> Result<Vec<Vec<u8>>, AppError> {
    let mut cursor = start;
    let mut out: Vec<Vec<u8>> = Vec::new();

    loop {
        let batch: Vec<Vec<u8>> = {
            let mut txn = txn.lock().await;
            txn.scan_keys(range_from(cursor.clone(), end.clone()), SCAN_BATCH)
                .await
                .map_err(to_app_error)?
                .map(Vec::from)
                .collect()
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
pub(super) async fn scan_prefix_keys(
    txn: &Mutex<Transaction>,
    prefix: &[u8],
) -> Result<Vec<Vec<u8>>, AppError> {
    scan_keys_range(txn, prefix.to_vec(), keys::prefix_end(prefix)).await
}

/// `start`（含む）から `end_inclusive`（含む）までのキーを取り出す。
pub(super) async fn scan_inclusive_keys(
    txn: &Mutex<Transaction>,
    start: Vec<u8>,
    end_inclusive: Vec<u8>,
) -> Result<Vec<Vec<u8>>, AppError> {
    // 終端を含めたいので、末尾に 0 を足して排他終端へ変換する。
    let mut end = end_inclusive;
    end.push(0);
    scan_keys_range(txn, start, Some(end)).await
}
