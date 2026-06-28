//! 動的シャードの構成要素。
//!
//! `tables_data` の各エントリ（キー = `(TableId, FlexId=シャード領域)`）は、
//! 実データを持つ **リーフ**（[`SpatialIdMap`](kasane_logic::SpatialIdMap) のバイト列）か、
//! 分割後に子シャードのキー（領域 [`FlexId`]）を **ポインタ** として並べた **中間ノード**のいずれか。
//!
//! リーフが [`MAX_FLEX_ID_PER_SHARD`] を超えると分割され、親エントリは子へのポインタノードへ置き換わる。
//! ルーティングはこのポインタを辿って対象リーフ（複数可）へ到達する。

use heed::types::Bytes;
use heed::{Database, RoTxn, WithoutTls};
use kasane_logic::{FlexId, SpatialIdMap};

use crate::db_init::TableIdAndFlexId;
use crate::error::AppError;
use crate::models::id::TableId;

/// 1 シャードが保持できる [`FlexId`] 数の上限。これを超えたシャードは動的に分割される。
pub const MAX_FLEX_ID_PER_SHARD: usize = 4096;

/// [`FlexId`] の `spatial_encode` のバイト長。
const FLEX_ID_LEN: usize = 14;

const TAG_LEAF: u8 = 0;
const TAG_POINTERS: u8 = 1;

/// `tables_data` の値の論理表現。
pub enum ShardEntry {
    /// 実データ（`SpatialIdMap` の rkyv バイト列）。
    Leaf(Vec<u8>),
    /// 子シャードの領域（=キー）へのポインタ群。
    Pointers(Vec<FlexId>),
}

impl ShardEntry {
    /// 生バイト列を解釈する。
    pub fn decode(bytes: &[u8]) -> Result<Self, AppError> {
        match bytes.first() {
            Some(&TAG_LEAF) => Ok(ShardEntry::Leaf(bytes[1..].to_vec())),
            Some(&TAG_POINTERS) => {
                let body = &bytes[1..];
                if !body.len().is_multiple_of(FLEX_ID_LEN) {
                    return Err(AppError::InternalError(
                        "invalid pointer node length".to_string(),
                    ));
                }
                let mut regions = Vec::with_capacity(body.len() / FLEX_ID_LEN);
                for chunk in body.chunks_exact(FLEX_ID_LEN) {
                    let mut b = [0u8; FLEX_ID_LEN];
                    b.copy_from_slice(chunk);
                    regions
                        .push(FlexId::spatial_decode(&b).map_err(|e| {
                            AppError::InternalError(format!("flex_id decode: {e}"))
                        })?);
                }
                Ok(ShardEntry::Pointers(regions))
            }
            _ => Err(AppError::InternalError("empty shard entry".to_string())),
        }
    }

    /// リーフ（`SpatialIdMap` バイト列）をエンコードする。
    pub fn encode_leaf(map_bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + map_bytes.len());
        out.push(TAG_LEAF);
        out.extend_from_slice(map_bytes);
        out
    }

    /// 子シャード領域へのポインタノードをエンコードする。
    pub fn encode_pointers(regions: &[FlexId]) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + regions.len() * FLEX_ID_LEN);
        out.push(TAG_POINTERS);
        for region in regions {
            out.extend_from_slice(&region.spatial_encode());
        }
        out
    }
}

// --- ルーティング・ロード（read / write 共用の自由関数） ---

/// `region` を根として `flex_id` と重なるリーフシャードの領域をすべて集める。
/// ポインタノードは再帰的に辿り、リーフ（または未作成のキー）を `out` に積む。
pub fn collect_leaf_regions(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    region: FlexId,
    flex_id: &FlexId,
    out: &mut Vec<FlexId>,
) -> Result<(), AppError> {
    match tables_data.get(txn, &(table_id, region.clone()))? {
        // 未作成 → ここが新規リーフになる。
        None => out.push(region),
        Some(bytes) => match ShardEntry::decode(bytes)? {
            ShardEntry::Leaf(_) => out.push(region),
            ShardEntry::Pointers(children) => {
                for child in children {
                    if child.intersection(flex_id).is_some() {
                        collect_leaf_regions(tables_data, txn, table_id, child, flex_id, out)?;
                    }
                }
            }
        },
    }
    Ok(())
}

/// `flex_id` が属する全リーフ領域を、f 符号で選んだ上下半球ルートから辿って返す。
pub fn route_leaves(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    flex_id: &FlexId,
) -> Result<Vec<FlexId>, AppError> {
    let root = if flex_id.f_index().is_negative() {
        FlexId::LOWER_MAX
    } else {
        FlexId::UPPER_MAX
    };
    let mut out = Vec::new();
    collect_leaf_regions(tables_data, txn, table_id, root, flex_id, &mut out)?;
    Ok(out)
}

/// リーフ領域 `region` の [`SpatialIdMap`] をロードする。未作成なら領域に閉じた空マップを返す。
pub fn load_leaf_map(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    region: &FlexId,
) -> Result<SpatialIdMap<Vec<u8>>, AppError> {
    match tables_data.get(txn, &(table_id, region.clone()))? {
        Some(bytes) => match ShardEntry::decode(bytes)? {
            ShardEntry::Leaf(map_bytes) => {
                unsafe { SpatialIdMap::<Vec<u8>>::from_bytes(&map_bytes) }
                    .map_err(|e| AppError::InternalError(format!("rkyv deserialize: {e}")))
            }
            ShardEntry::Pointers(_) => Err(AppError::InternalError(
                "routed to a pointer node".to_string(),
            )),
        },
        None => Ok(SpatialIdMap::new_in_shard(region.clone())),
    }
}
