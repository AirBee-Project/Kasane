//! シャードツリーの探索とノードのロード。

use heed::types::Bytes;
use heed::{Database, RoTxn, WithoutTls};
use spatial_id::{FlexId, collection::SpatialIdMap};

use super::keys::TableIdAndFlexId;
use crate::error::AppError;
use crate::models::id::TableId;

pub use crate::repositories::encoding::shard_entry::{
    LeafView, MAX_FLEX_ID_PER_SHARD, MERGE_FLEX_ID_THRESHOLD, ShardEntry,
};

/// 複数の `flex_id` を一度の木降下で `担当リーフ領域 -> 到達した flex_id 群` へ振り分ける。
pub fn route_leaves_batched<'a>(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    ids: impl Iterator<Item = &'a FlexId>,
) -> Result<rustc_hash::FxHashMap<FlexId, Vec<FlexId>>, AppError> {
    // f 符号で上下半球に分け、各半球ルートから 1 回ずつ降りる。
    let mut lower = Vec::new();
    let mut upper = Vec::new();
    for f in ids {
        if f.f_index().is_negative() {
            lower.push(f);
        } else {
            upper.push(f);
        }
    }

    let mut out: rustc_hash::FxHashMap<FlexId, Vec<FlexId>> = rustc_hash::FxHashMap::default();
    descend_batched(
        tables_data,
        txn,
        table_id,
        FlexId::LOWER_MAX,
        lower,
        &mut out,
    )?;
    descend_batched(
        tables_data,
        txn,
        table_id,
        FlexId::UPPER_MAX,
        upper,
        &mut out,
    )?;
    Ok(out)
}

/// [`route_leaves_batched`] と違い未作成領域は返さない（読み取り専用の経路なので）。
pub fn route_leaves_for_range(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    range: &spatial_id::RangeId,
) -> Result<Vec<FlexId>, AppError> {
    let mut out = Vec::new();
    for root in [FlexId::LOWER_MAX, FlexId::UPPER_MAX] {
        if root.intersects_range(range) {
            descend_range(tables_data, txn, table_id, root, range, &mut out)?;
        }
    }
    Ok(out)
}

fn descend_range(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    region: FlexId,
    range: &spatial_id::RangeId,
    out: &mut Vec<FlexId>,
) -> Result<(), AppError> {
    let Some(bytes) = tables_data.get(txn, &(table_id, region))? else {
        // 未作成領域＝データ無し。読み取りでは辿る必要がない。
        return Ok(());
    };
    match ShardEntry::child_pointers(bytes)? {
        None => out.push(region),
        Some(children) => {
            for child in children {
                if child.intersects_range(range) {
                    descend_range(tables_data, txn, table_id, child, range, out)?;
                }
            }
        }
    }
    Ok(())
}

fn descend_batched<'a>(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    region: FlexId,
    ids: Vec<&'a FlexId>,
    out: &mut rustc_hash::FxHashMap<FlexId, Vec<FlexId>>,
) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    match tables_data.get(txn, &(table_id, region))? {
        // 未作成領域と実データリーフのどちらも、到達した全 flex_id がここの担当。
        // リーフの外へはみ出した部分を書かないよう、領域で切り取って渡す。
        None => {
            out.entry(region)
                .or_default()
                .extend(ids.into_iter().filter_map(|f| f.intersection(&region)));
        }
        Some(bytes) => match ShardEntry::child_pointers(bytes)? {
            None => {
                out.entry(region)
                    .or_default()
                    .extend(ids.into_iter().filter_map(|f| f.intersection(&region)));
            }
            Some(children) => {
                for child in children {
                    let bucket: Vec<&'a FlexId> = ids
                        .iter()
                        .filter(|&&f| child.intersection(f).is_some())
                        .copied()
                        .collect();
                    descend_batched(tables_data, txn, table_id, child, bucket, out)?;
                }
            }
        },
    }
    Ok(())
}

/// `region` がルート、またはポインタ配下でないなら `None`。
pub fn find_parent_pointer(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    region: &FlexId,
) -> Result<Option<(FlexId, Vec<FlexId>)>, AppError> {
    let root = if region.f_index().is_negative() {
        FlexId::LOWER_MAX
    } else {
        FlexId::UPPER_MAX
    };
    if region == &root {
        return Ok(None);
    }

    let mut cur = root;
    loop {
        match tables_data.get(txn, &(table_id, cur))? {
            Some(bytes) => match ShardEntry::child_pointers(bytes)? {
                Some(children) => {
                    if children.iter().any(|c| c == region) {
                        return Ok(Some((cur, children)));
                    }
                    match children
                        .into_iter()
                        .find(|c| c.intersection(region) == Some(*region))
                    {
                        Some(child) => cur = child,
                        None => return Ok(None),
                    }
                }
                None => return Ok(None),
            },
            None => return Ok(None),
        }
    }
}

/// リーフを、値を mmap から複製せずに開く（木を組み直さない）。読み取り専用。
pub fn load_leaf_view<'txn>(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &'txn RoTxn<WithoutTls>,
    table_id: TableId,
    region: &FlexId,
) -> Result<Option<LeafView<'txn>>, AppError> {
    match tables_data.get(txn, &(table_id, *region))? {
        Some(entry) => match ShardEntry::leaf_payload(entry)? {
            Some(map_bytes) => Ok(Some(LeafView::parse(map_bytes)?)),
            None => Err(AppError::Internal("routed to a pointer node".to_string())),
        },
        None => Ok(None),
    }
}

/// 書き換え用に [`SpatialIdMap`] を復元する。読むだけなら [`load_leaf_view`]。
pub fn load_leaf_map(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    region: &FlexId,
) -> Result<SpatialIdMap<Vec<u8>>, AppError> {
    match tables_data.get(txn, &(table_id, *region))? {
        Some(bytes) => match ShardEntry::decode(bytes)? {
            ShardEntry::Leaf(map_bytes) => Ok(LeafView::parse(&map_bytes)?.to_map()),
            ShardEntry::Pointers(_) => {
                Err(AppError::Internal("routed to a pointer node".to_string()))
            }
        },
        None => Ok(SpatialIdMap::new()),
    }
}
