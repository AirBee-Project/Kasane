//! リーフの書き換え（純粋な計算）。**ここには await が 1 つも無い。**
//!
//! 丸ごと blocking タスクへ移せるようにするため。リーフ 1 枚ごとに rkyv の復元と直列化が
//! 走るので、非同期ワーカー上に置くと **tikv-client のハートビート**までワーカーを待ち、
//! ロックが TTL 切れで回収される。つまり負荷が上がったときに書き込みが落ちる。

use kasane_logic::{FlexId, SpatialIdMap, SpatialIdSet};
use rustc_hash::{FxHashMap, FxHashSet};

use super::node::decode_leaf;
use super::routing::RoutedLeaf;
use super::{
    AppError, MAX_FLEX_ID_PER_SHARD, ShardEntry, ShardValue, TableDataType, TableId, keys, kv,
    value_index,
};

/// 値を借用ではなく所有で持つのは、この操作ごと blocking タスクへ move するため。
pub(super) enum LeafOp {
    /// 既存値は上書き。
    Insert(Vec<u8>),
    /// まだ値の無い空間 ID にだけ書く。
    Upsert(Vec<u8>),
    Remove,
    /// 空間 ID ごとに別々の値を書く。
    InsertMany(BatchWrite),
}

/// `owner` が `flex_id → values` の添字を持つので、走査中はハッシュ 1 回で書く値が決まる。
///
/// 要素ごとに空間 ID 集合を持ち回って毎回交差を取ると、要素数 × リーフの FlexId 数になる。
pub(super) struct BatchWrite {
    /// 同じ空間 ID が複数回指定された場合は**後勝ち**。
    owner: FxHashMap<FlexId, u32>,
    values: Vec<Vec<u8>>,
}

impl BatchWrite {
    /// 書き込み対象の全 `FlexId` も返す。
    pub(super) fn new(entries: Vec<(SpatialIdSet, Vec<u8>)>) -> (Self, Vec<FlexId>) {
        let mut owner: FxHashMap<FlexId, u32> = FxHashMap::default();
        let mut values = Vec::with_capacity(entries.len());
        for (ids, value) in entries {
            let slot = values.len() as u32;
            values.push(value);
            for flex_id in ids.flex_ids() {
                // 1 件ずつ順に書いたときと同じ結果にする。
                owner.insert(flex_id, slot);
            }
        }
        let targets = owner.keys().copied().collect();
        (Self { owner, values }, targets)
    }
}

impl LeafOp {
    fn apply(&self, map: &mut SpatialIdMap<Vec<u8>>, targets: &[FlexId]) {
        match self {
            LeafOp::Insert(data) => {
                for flex_id in targets {
                    map.insert(*flex_id, data.clone());
                }
            }
            LeafOp::Upsert(data) => {
                let mut target_set = SpatialIdSet::new();
                for flex_id in targets {
                    let occupied: SpatialIdSet = map.get(flex_id).map(|(f, _)| f).collect();
                    target_set.clear();
                    target_set.insert(*flex_id);
                    for f in (&target_set - &occupied).flex_ids() {
                        map.insert(f, data.clone());
                    }
                }
            }
            LeafOp::Remove => {
                for flex_id in targets {
                    map.remove(flex_id);
                }
            }
            LeafOp::InsertMany(batch) => {
                for flex_id in targets {
                    // 降下で振り分けられた ID は必ず台帳にある。無ければ他所の ID。
                    if let Some(&slot) = batch.owner.get(flex_id) {
                        map.insert(*flex_id, batch.values[slot as usize].clone());
                    }
                }
            }
        }
    }
}

/// 索引キーは格納 `FlexId` 1 件につき 1 つ増えるので、`index` の有無で触るキー数が 3 桁変わる。
pub(super) fn apply_leaf(
    table_id: TableId,
    index: Option<TableDataType>,
    leaf: &RoutedLeaf,
    op: &LeafOp,
    out: &mut Vec<kv::Mutation>,
) -> Result<(), AppError> {
    let mut map = decode_leaf(&leaf.region, leaf.node.as_ref().map(ShardValue::entry))?;
    let targets = &leaf.queries;

    let Some(data_type) = index else {
        op.apply(&mut map, targets);
        return store_shard(table_id, leaf.region, map, out);
    };

    let scan: SpatialIdSet = targets.iter().cloned().collect();

    let mut old_keys = FxHashSet::default();
    let mut pre_modify_scan = scan.clone();
    for f_scan in scan.iter() {
        for (f, v) in map.get_overlapping(&f_scan) {
            old_keys.insert(index_key(table_id, data_type, v, &f));
            pre_modify_scan.insert(f);
        }
    }

    op.apply(&mut map, targets);

    let mut new_keys = FxHashSet::default();
    for f_scan in pre_modify_scan.iter() {
        for (f, v) in map.get_overlapping(&f_scan) {
            new_keys.insert(index_key(table_id, data_type, v, &f));
        }
    }

    kv::value_index_mutations(&old_keys, &new_keys, out);
    store_shard(table_id, leaf.region, map, out)
}

/// 変更後のリーフを保存する。過大なら分割し、空なら削除する。
fn store_shard(
    table_id: TableId,
    region: FlexId,
    map: SpatialIdMap<Vec<u8>>,
    out: &mut Vec<kv::Mutation>,
) -> Result<(), AppError> {
    if !map.should_split_shard(MAX_FLEX_ID_PER_SHARD) {
        if map.is_empty() {
            out.extend(kv::shard_deletions(table_id, &region));
        } else {
            put_leaf(table_id, &region, &map, out)?;
        }
        return Ok(());
    }

    // 子へ他者が到達する経路は無い（親をポインタノードにするのがこの処理自身）。
    let mut children = Vec::new();
    let ((lo_r, lo), (hi_r, hi)) = map
        .split_shard()
        .ok_or_else(|| AppError::InternalError("split on shardless map".to_string()))?;
    emit_child(table_id, lo_r, lo, &mut children, out)?;
    emit_child(table_id, hi_r, hi, &mut children, out)?;

    out.extend(kv::shard_mutations(
        table_id,
        &region,
        &ShardEntry::encode_pointers(&children),
    )?);
    Ok(())
}

/// パス圧縮の本体。片側が空になる分割（退化分割）で中間ポインタを作らないので、実際に
/// データが分かれる軸でだけポインタノードができて木が浅く保たれる。
fn emit_child(
    table_id: TableId,
    cr: FlexId,
    cm: SpatialIdMap<Vec<u8>>,
    covers: &mut Vec<FlexId>,
    out: &mut Vec<kv::Mutation>,
) -> Result<(), AppError> {
    if cm.is_empty() {
        // 空領域：被覆として領域だけ積む。万一の古いキーは消す。
        out.extend(kv::shard_deletions(table_id, &cr));
        covers.push(cr);
        return Ok(());
    }
    if !cm.should_split_shard(MAX_FLEX_ID_PER_SHARD) {
        put_leaf(table_id, &cr, &cm, out)?;
        covers.push(cr);
        return Ok(());
    }

    // 1 段だけ覗いて、退化分割か実分割かを決める。
    let ((clo_r, clo), (chi_r, chi)) = cm
        .split_shard()
        .ok_or_else(|| AppError::InternalError("split on shardless map".to_string()))?;

    if clo.is_empty() || chi.is_empty() {
        // 退化分割：中間ポインタを作らず孫を巻き上げる。
        emit_child(table_id, clo_r, clo, covers, out)?;
        emit_child(table_id, chi_r, chi, covers, out)?;
    } else {
        let mut grand = Vec::new();
        emit_child(table_id, clo_r, clo, &mut grand, out)?;
        emit_child(table_id, chi_r, chi, &mut grand, out)?;
        out.extend(kv::shard_mutations(
            table_id,
            &cr,
            &ShardEntry::encode_pointers(&grand),
        )?);
        covers.push(cr);
    }
    Ok(())
}

/// 件数ヘッダ付きで保存する。
fn put_leaf(
    table_id: TableId,
    region: &FlexId,
    map: &SpatialIdMap<Vec<u8>>,
    out: &mut Vec<kv::Mutation>,
) -> Result<(), AppError> {
    let bytes = map
        .to_bytes()
        .map_err(|e| AppError::InternalError(format!("rkyv serialize: {e}")))?;
    out.extend(kv::shard_mutations(
        table_id,
        region,
        &ShardEntry::encode_leaf(map.count() as u32, &bytes),
    )?);
    Ok(())
}

/// 子のバイト列はロック時に手元へ来ているので引き直さない。
pub(super) fn merge_children(
    table_id: TableId,
    index: Option<TableDataType>,
    parent_region: FlexId,
    child_regions: &[FlexId],
    locked: &std::collections::BTreeMap<FlexId, Option<ShardValue>>,
) -> Result<Vec<kv::Mutation>, AppError> {
    let mut child_maps: Vec<SpatialIdMap<Vec<u8>>> = Vec::new();
    for cr in child_regions {
        let entry = locked.get(cr).and_then(|v| v.as_ref());
        let map = decode_leaf(cr, entry.map(ShardValue::entry))?;
        if !map.is_empty() {
            child_maps.push(map);
        }
    }

    // 統合は木の形を変えるだけで、`(FlexId, 値)` の対応は親へ移っても変わらない。
    let old_keys = index.map(|data_type| {
        let mut keys = FxHashSet::default();
        for m in &child_maps {
            for (f, v) in m.iter() {
                keys.insert(index_key(table_id, data_type, v, &f));
            }
        }
        keys
    });

    let merged = SpatialIdMap::merge_shards(parent_region, child_maps)?;

    let mut out = Vec::new();
    if let (Some(data_type), Some(old_keys)) = (index, old_keys) {
        let mut new_keys = FxHashSet::default();
        for (f, v) in merged.iter() {
            new_keys.insert(index_key(table_id, data_type, v, &f));
        }
        kv::value_index_mutations(&old_keys, &new_keys, &mut out);
    }

    if merged.is_empty() {
        out.extend(kv::shard_deletions(table_id, &parent_region));
    } else {
        put_leaf(table_id, &parent_region, &merged, &mut out)?;
    }
    for cr in child_regions {
        out.extend(kv::shard_deletions(table_id, cr));
    }
    Ok(out)
}

fn index_key(
    table_id: TableId,
    data_type: TableDataType,
    value: &[u8],
    flex_id: &FlexId,
) -> Vec<u8> {
    keys::value_index(
        table_id,
        &value_index::order_preserving(data_type, value),
        flex_id,
    )
}
