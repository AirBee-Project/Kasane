//! ノード 1 枚の読み書き。木の中で最も下の層。

use kasane_logic::{ArchivedSpatialIdMap, FlexId, SpatialIdMap};
use rustc_hash::FxHashMap;

use super::{AppError, Reader, Readers, ShardEntry, ShardValue, TableId, keys, kv};

// --- ノードの読み書き ---

/// 存在しない領域は結果に含まれない。
///
/// 戻ってきたキーから領域を復元するのは、対応表を作ると領域の数だけ「キーの複製」と「バイト列
/// のハッシュ」が乗るため。降下で 1 段につき 1 回呼ばれ、扱う領域数はテーブルが大きいほど増える。
pub(super) async fn load_nodes<R: Reader>(
    txn: &Readers<R>,
    table_id: TableId,
    regions: &[FlexId],
) -> Result<FxHashMap<FlexId, ShardValue>, AppError> {
    let keys: Vec<Vec<u8>> = regions.iter().map(|r| keys::shard(table_id, r)).collect();
    kv::batch_get_shards(txn, keys)
        .await?
        .into_iter()
        .map(|(key, value)| Ok((keys::region_from_shard_key(&key)?, value)))
        .collect()
}

/// **書き換えるとき専用。** 読むだけなら [`archived_leaf`]。未作成なら空のマップ。
pub(super) fn decode_leaf(
    region: &FlexId,
    entry: Option<&[u8]>,
) -> Result<SpatialIdMap<Vec<u8>>, AppError> {
    let Some(entry) = entry else {
        return Ok(SpatialIdMap::new_in_shard(*region));
    };
    match ShardEntry::leaf_payload(entry)? {
        // SAFETY: CRC 検証済みのバイト列。形式バージョンは `from_bytes` が検証する。
        Some(map_bytes) => unsafe { SpatialIdMap::<Vec<u8>>::from_bytes(map_bytes) }
            .map_err(|e| AppError::InternalError(format!("rkyv deserialize: {e}"))),
        None => Err(AppError::InternalError(
            "routed to a pointer node".to_string(),
        )),
    }
}

/// 読み取り専用。
pub(super) fn archived_leaf(entry: &[u8]) -> Result<ArchivedSpatialIdMap<'_>, AppError> {
    match ShardEntry::leaf_payload(entry)? {
        // SAFETY: `decode_leaf` と同じ根拠。
        Some(map_bytes) => unsafe { ArchivedSpatialIdMap::access(map_bytes) }
            .map_err(|e| AppError::InternalError(format!("leaf format: {e}"))),
        None => Err(AppError::InternalError(
            "routed to a pointer node".to_string(),
        )),
    }
}
