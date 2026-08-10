use heed::types::Bytes;
use heed::{Database, RoTxn, WithoutTls};
use kasane_logic::{ArchivedSpatialIdMap, FlexId, SpatialIdMap};

use crate::db_init::TableIdAndFlexId;
use crate::error::AppError;
use crate::models::id::TableId;

// ノードのバイト表現はバックエンド非依存なので共有モジュールを使う。
// ここに残るのは heed のトランザクションを必要とする探索・ロード処理だけ。
pub use crate::repositories::encoding::shard_entry::{
    MAX_FLEX_ID_PER_SHARD, MERGE_FLEX_ID_THRESHOLD, ShardEntry,
};

// --- ルーティング・ロード（read / write 共用の自由関数） ---

/// 複数の `flex_id` を**一度の木降下**でまとめてルーティングし、
/// `担当リーフ領域 -> そこへ到達した flex_id 群` を返す。
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

/// `range` と重なる**既存のリーフ領域**を、ポインタ木を1回降りて集める。
///
/// [`route_leaves_batched`] が FlexId 群を担当リーフへ振り分ける（書き込み用に未作成キーも返す）のに対し、
/// こちらは範囲クエリ用の読み取り経路であり、**データが存在するリーフだけ**を返す。
pub fn route_leaves_for_range(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    range: &kasane_logic::RangeId,
) -> Result<Vec<FlexId>, AppError> {
    let mut out = Vec::new();
    for root in [FlexId::LOWER_MAX, FlexId::UPPER_MAX] {
        if root.intersects_range(range) {
            descend_range(tables_data, txn, table_id, root, range, &mut out)?;
        }
    }
    Ok(out)
}

/// `region` を根として、`range` と交差する子だけを辿り、到達したリーフ領域を `out` に積む。
fn descend_range(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    region: FlexId,
    range: &kasane_logic::RangeId,
    out: &mut Vec<FlexId>,
) -> Result<(), AppError> {
    let Some(bytes) = tables_data.get(txn, &(table_id, region))? else {
        // 未作成領域＝データ無し。読み取りでは辿る必要がない。
        return Ok(());
    };
    match ShardEntry::child_pointers(bytes)? {
        // リーフに到達。
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

/// `region` を根として `ids` を子へ振り分けながら降り、リーフ（または未作成キー）へ到達した
/// flex_id 群を `out` に積む。
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
        // 未作成リーフ or 実データリーフ → ここへ到達した全 flex_id が担当。
        None => {
            out.entry(region)
                .or_default()
                .extend(ids.into_iter().cloned());
        }
        Some(bytes) => match ShardEntry::child_pointers(bytes)? {
            None => {
                out.entry(region)
                    .or_default()
                    .extend(ids.into_iter().cloned());
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

/// `region`を直接の子に持つ**親ポインタノード**を見つけ、`(親領域, 親の全子領域)` を返す。
/// `region`が `0/0/0/0` or `0/-1/0/0`などのルートや、ポインタ配下でないなら `None`。
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
    // ルート自身に親はない。
    if region == &root {
        return Ok(None);
    }

    let mut cur = root;
    loop {
        match tables_data.get(txn, &(table_id, cur))? {
            Some(bytes) => match ShardEntry::child_pointers(bytes)? {
                Some(children) => {
                    // region が直接の子なら、cur が親。
                    if children.iter().any(|c| c == region) {
                        return Ok(Some((cur, children)));
                    }
                    // region を含む子へ降りる（region ⊆ child）。
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

/// リーフ領域 `region` を **ZeroCopy archived リーダ**として開く（`Arc`を再構築しない）。
/// 未作成なら `Ok(None)`、ポインタノードに当たったら `Err`。読み取り専用。
pub fn load_leaf_archived<'txn>(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &'txn RoTxn<WithoutTls>,
    table_id: TableId,
    region: &FlexId,
) -> Result<Option<ArchivedSpatialIdMap<'txn>>, AppError> {
    match tables_data.get(txn, &(table_id, *region))? {
        Some(entry) => match ShardEntry::leaf_payload(entry)? {
            // 自分自身の to_bytes が書いた正当なバイト列。
            // 形式バージョンだけは検証されるので、古い形式のデータは黙って誤読されず
            // ここでエラーになる。
            Some(map_bytes) => Ok(Some(
                unsafe { ArchivedSpatialIdMap::access(map_bytes) }
                    .map_err(|e| AppError::InternalError(format!("leaf format: {e}")))?,
            )),
            None => Err(AppError::InternalError(
                "routed to a pointer node".to_string(),
            )),
        },
        None => Ok(None),
    }
}

/// リーフ領域 `region` の [`SpatialIdMap`] をロードする。未作成なら空の[`SpatialIdMap`]を作成する。
pub fn load_leaf_map(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    region: &FlexId,
) -> Result<SpatialIdMap<Vec<u8>>, AppError> {
    match tables_data.get(txn, &(table_id, *region))? {
        Some(bytes) => match ShardEntry::decode(bytes)? {
            ShardEntry::Leaf(map_bytes) => {
                unsafe { SpatialIdMap::<Vec<u8>>::from_bytes(&map_bytes) }
                    .map_err(|e| AppError::InternalError(format!("rkyv deserialize: {e}")))
            }
            ShardEntry::Pointers(_) => Err(AppError::InternalError(
                "routed to a pointer node".to_string(),
            )),
        },
        None => Ok(SpatialIdMap::new_in_shard(*region)),
    }
}
