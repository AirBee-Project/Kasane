//! ノード 1 枚の読み書き。木の中で**最も下**の層で、ここだけが `kv` に触れる。

use kasane_logic::{ArchivedSpatialIdMap, FlexId, SpatialIdMap};
use rustc_hash::FxHashMap;

use super::{AppError, Reader, Readers, ShardEntry, ShardValue, TableId, keys, kv};

// --- ノードの読み書き ---

/// 複数領域のノードをまとめて取得する。存在しない領域は結果に含まれない。
///
/// 呼び出し側はキーではなく領域で引きたいので、領域をキーにして返す
/// （キーで返すと、引くたびにキーを組み立て直すことになる）。
///
/// 戻ってきたキーからは [`keys::region_from_shard_key`] で領域を復元する。
/// キー → 領域の対応表を作る手もあるが、そうすると領域の数だけ
/// 「キーの複製」と「バイト列のハッシュ」が乗る。この関数は木の降下で**1 段につき
/// 1 回**呼ばれ、1 回で扱う領域数はテーブルが大きいほど増えるので、そこは削っておく。
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

/// リーフのバイト列から [`SpatialIdMap`] を復元する。未作成なら空のマップ。
///
/// **書き換えるとき専用。** 読むだけなら [`archived_leaf`] を使う。
pub(super) fn decode_leaf(
    region: &FlexId,
    entry: Option<&[u8]>,
) -> Result<SpatialIdMap<Vec<u8>>, AppError> {
    let Some(entry) = entry else {
        return Ok(SpatialIdMap::new_in_shard(*region));
    };
    match ShardEntry::leaf_payload(entry)? {
        // SAFETY: `entry` は `kv::ShardValue` の CRC 検証を通ったバイト列で、
        // 保存時に自分が書いたものと一致することが確認済み。形式バージョンは
        // `from_bytes` 側でさらに検証されるので、古い形式が黙って誤読されることもない。
        Some(map_bytes) => unsafe { SpatialIdMap::<Vec<u8>>::from_bytes(map_bytes) }
            .map_err(|e| AppError::InternalError(format!("rkyv deserialize: {e}"))),
        None => Err(AppError::InternalError(
            "routed to a pointer node".to_string(),
        )),
    }
}

/// リーフのバイト列をゼロコピーで開く（読み取り専用）。
pub(super) fn archived_leaf(entry: &[u8]) -> Result<ArchivedSpatialIdMap<'_>, AppError> {
    match ShardEntry::leaf_payload(entry)? {
        // SAFETY: `decode_leaf` と同じ根拠（CRC 検証済みのバイト列）。形式バージョンは
        // `access` 側でさらに検証される。
        Some(map_bytes) => unsafe { ArchivedSpatialIdMap::access(map_bytes) }
            .map_err(|e| AppError::InternalError(format!("leaf format: {e}"))),
        None => Err(AppError::InternalError(
            "routed to a pointer node".to_string(),
        )),
    }
}
