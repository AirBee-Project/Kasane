use rustc_hash::FxHashMap;

use spatial_id::{
    FlexId,
    collection::{SpatialIdMap, SpatialIdSet},
};

use crate::error::AppError;
use crate::models::id::TableId;
use crate::repositories::ValueGroups;
use crate::repositories::encoding::shard_entry::{
    MAX_FLEX_ID_PER_SHARD, MAX_SHARD_BYTES, MERGE_FLEX_ID_THRESHOLD, ShardEntry, encode_map,
    shard_needs_split,
};

use super::shard;
use super::{LmdbRead, LmdbWrite};

type ValueMap = FxHashMap<Vec<u8>, Vec<FlexId>>;

#[tracing::instrument(skip_all)]
pub(super) fn data_get(
    r: &LmdbRead<'_>,
    table_id: crate::models::id::TableId,
    ids: SpatialIdSet,
) -> Result<ValueGroups, AppError> {
    let mut flex_ids_iter = ids.iter();
    let mut global_by_value = ValueMap::default();

    // 数百万件を一度にルーティングへ渡すとその分のメモリを一度に確保するので、区切る。
    const ROUTING_BATCH_SIZE: usize = 65536;

    loop {
        let mut batch = Vec::with_capacity(ROUTING_BATCH_SIZE.min(1024));
        for _ in 0..ROUTING_BATCH_SIZE {
            if let Some(id) = flex_ids_iter.next() {
                batch.push(id);
            } else {
                break;
            }
        }
        if batch.is_empty() {
            break;
        }

        let by_leaf =
            shard::route_leaves_batched(&r.db.tables_data, &r.read_txn, table_id, batch.iter())?;
        for (region, queries) in by_leaf {
            resolve_leaf(
                &r.db.tables_data,
                &r.read_txn,
                table_id,
                &region,
                &queries,
                &mut global_by_value,
            )?;
        }
    }

    Ok(global_by_value.into_iter().collect())
}

/// 1 枚のリーフを走査し、`by_value`（値 → FlexId 群）へ積む。
///
/// 葉ローカルの辞書インデックス（`u32`）で先にグルーピングするのは、値バイト列で直接
/// ハッシュすると結果 FlexId の数だけ長いバイト列をハッシュすることになるため。
fn resolve_leaf(
    tables_data: &heed::Database<super::keys::TableIdAndFlexId, heed::types::Bytes>,
    txn: &heed::RoTxn<heed::WithoutTls>,
    table_id: TableId,
    region: &FlexId,
    queries: &[FlexId],
    by_value: &mut ValueMap,
) -> Result<(), AppError> {
    let Some(leaf) = shard::load_leaf_view(tables_data, txn, table_id, region)? else {
        return Ok(());
    };

    let mut local: FxHashMap<u32, Vec<FlexId>> = FxHashMap::default();
    for query_flex in queries {
        for &(id, index) in &leaf.entries {
            if let Some(got_flex) = id.intersection(query_flex) {
                local.entry(index).or_default().push(got_flex);
            }
        }
    }

    // 葉に現れた distinct 値の数だけ実バイト列へ復元する。
    for (index, mut flex_ids) in local {
        let value = leaf.value(index);
        if let Some(existing) = by_value.get_mut(value) {
            existing.append(&mut flex_ids);
        } else {
            by_value.insert(value.to_vec(), flex_ids);
        }
    }
    Ok(())
}

#[tracing::instrument(skip_all)]
pub(super) fn data_insert(
    w: &mut LmdbWrite<'_>,
    table_id: TableId,
    ids: SpatialIdSet,
    data: &[u8],
) -> Result<(), AppError> {
    let by_leaf = w.group_by_leaf(table_id, ids.iter())?;
    for (region, flex_ids) in by_leaf {
        let map = shard::load_leaf_map(&w.db.tables_data, &w.write_txn, table_id, &region)?;
        w.apply_leaf(table_id, region, map, |m| {
            for flex_id in &flex_ids {
                m.insert(*flex_id, data.to_vec());
            }
        })?;
    }
    Ok(())
}

/// まだ値が無い空間 ID にだけ書く（既存値は保持）。
#[tracing::instrument(skip_all)]
pub(super) fn data_upsert(
    w: &mut LmdbWrite<'_>,
    table_id: TableId,
    ids: SpatialIdSet,
    data: &[u8],
) -> Result<(), AppError> {
    let by_leaf = w.group_by_leaf(table_id, ids.iter())?;
    let data_vec = data.to_vec();
    for (region, flex_ids) in by_leaf {
        let map = shard::load_leaf_map(&w.db.tables_data, &w.write_txn, table_id, &region)?;
        w.apply_leaf(table_id, region, map, |m| {
            let mut target_set = SpatialIdSet::new();
            for flex_id in &flex_ids {
                let occupied_set: SpatialIdSet = m.get(*flex_id).map(|(f, _)| f).collect();
                target_set.clear();
                target_set.insert(*flex_id);

                for f in (&target_set - &occupied_set).iter() {
                    m.insert(f, data_vec.clone());
                }
            }
        })?;
    }
    Ok(())
}

#[tracing::instrument(skip_all)]
pub(super) fn data_remove(
    w: &mut LmdbWrite<'_>,
    table_id: TableId,
    ids: SpatialIdSet,
) -> Result<(), AppError> {
    let by_leaf = w.group_by_leaf(table_id, ids.iter())?;
    let mut affected: Vec<FlexId> = Vec::new();
    for (region, flex_ids) in by_leaf {
        let map = shard::load_leaf_map(&w.db.tables_data, &w.write_txn, table_id, &region)?;
        w.apply_leaf(table_id, region, map, |m| {
            for flex_id in &flex_ids {
                m.remove(*flex_id);
            }
        })?;
        affected.push(region);
    }
    for region in affected {
        w.try_merge_up(table_id, region)?;
    }
    Ok(())
}

impl<'a> LmdbWrite<'a> {
    /// 削除でデータ量が減ったリーフを親へ統合し、可能な限り木を圧縮する。
    fn try_merge_up(&mut self, table_id: TableId, region: FlexId) -> Result<(), AppError> {
        let mut region = region;
        while let Some((parent_region, child_regions)) =
            shard::find_parent_pointer(&self.db.tables_data, &self.write_txn, table_id, &region)?
        {
            // 子のいずれかがポインタノードなら、このレベルは統合しない。
            // バイト数も見るのは、件数は少なくても値が大きい葉同士を統合して
            // MAX_SHARD_BYTES 超の葉を作ってしまわないため。
            let mut combined = 0usize;
            let mut combined_bytes = 0usize;
            let mut mergeable = true;
            for cr in &child_regions {
                // 空領域のキーはそもそも存在しない。
                let Some(bytes) = self.db.tables_data.get(&self.write_txn, &(table_id, *cr))?
                else {
                    continue;
                };
                match ShardEntry::leaf_count(bytes)? {
                    Some(count) => {
                        combined += count as usize;
                        combined_bytes += bytes.len();
                        if combined > MERGE_FLEX_ID_THRESHOLD || combined_bytes > MAX_SHARD_BYTES {
                            mergeable = false;
                            break;
                        }
                    }
                    None => {
                        mergeable = false;
                        break;
                    }
                }
            }
            if !mergeable {
                break;
            }

            // 統合できると決まってから復元する（復元は重い）。
            let mut child_maps: Vec<SpatialIdMap<Vec<u8>>> = Vec::new();
            for cr in &child_regions {
                let map =
                    shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, cr)?;
                if !map.is_empty() {
                    child_maps.push(map);
                }
            }

            let merged: SpatialIdMap<Vec<u8>> = child_maps.into_iter().flatten().collect();

            let parent_key = (table_id, parent_region);
            if merged.is_empty() {
                self.db
                    .tables_data
                    .delete(&mut self.write_txn, &parent_key)?;
            } else {
                self.put_leaf(table_id, &parent_region, &merged)?;
            }
            for cr in &child_regions {
                self.db
                    .tables_data
                    .delete(&mut self.write_txn, &(table_id, *cr))?;
            }

            // 親が新たなリーフになったので、さらに上へ伝播する。
            region = parent_region;
        }
        Ok(())
    }

    fn apply_leaf(
        &mut self,
        table_id: TableId,
        region: FlexId,
        mut map: SpatialIdMap<Vec<u8>>,
        modify: impl FnOnce(&mut SpatialIdMap<Vec<u8>>),
    ) -> Result<(), AppError> {
        modify(&mut map);
        self.store_shard(table_id, region, map)
    }

    fn group_by_leaf(
        &self,
        table_id: TableId,
        flex_ids: impl Iterator<Item = FlexId>,
    ) -> Result<FxHashMap<FlexId, Vec<FlexId>>, AppError> {
        let ids: Vec<FlexId> = flex_ids.collect();
        shard::route_leaves_batched(&self.db.tables_data, &self.write_txn, table_id, ids.iter())
    }

    /// 呼ばれる時点で `region` は必ずリーフ（または未作成領域）である。
    fn store_shard(
        &mut self,
        table_id: TableId,
        region: FlexId,
        map: SpatialIdMap<Vec<u8>>,
    ) -> Result<(), AppError> {
        let key = (table_id, region);

        if map.is_empty() {
            self.db.tables_data.delete(&mut self.write_txn, &key)?;
            return Ok(());
        }

        let count = map.count();
        if count <= MAX_FLEX_ID_PER_SHARD {
            let bytes = encode_map(&map);
            if !shard_needs_split(count, bytes.len()) {
                self.put_leaf_bytes(table_id, &region, count as u32, &bytes)?;
                return Ok(());
            }
        }

        // 分割が必要 → パス圧縮した被覆子領域を構築し、親をポインタノードにする。
        let mut children = Vec::new();
        self.cover_split(table_id, region, &map, &mut children)?;
        self.db.tables_data.put(
            &mut self.write_txn,
            &key,
            &ShardEntry::encode_pointers(&children),
        )?;
        Ok(())
    }

    fn cover_split(
        &mut self,
        table_id: TableId,
        region: FlexId,
        map: &SpatialIdMap<Vec<u8>>,
        out: &mut Vec<FlexId>,
    ) -> Result<(), AppError> {
        let ((lo_r, lo), (hi_r, hi)) = map.split_shard(region).ok_or_else(|| {
            AppError::Internal("shard region cannot be split further".to_string())
        })?;
        self.emit_child(table_id, lo_r, lo, out)?;
        self.emit_child(table_id, hi_r, hi, out)?;
        Ok(())
    }

    /// パス圧縮の本体。片側が空になる分割（退化分割）で中間ポインタを作らないので、
    /// 実際にデータが分かれる軸でだけポインタノードができて木が浅く保たれる。
    fn emit_child(
        &mut self,
        table_id: TableId,
        cr: FlexId,
        cm: SpatialIdMap<Vec<u8>>,
        out: &mut Vec<FlexId>,
    ) -> Result<(), AppError> {
        if cm.is_empty() {
            // 空領域：被覆として領域だけ積む。万一の古いキーは消す。
            self.db
                .tables_data
                .delete(&mut self.write_txn, &(table_id, cr))?;
            out.push(cr);
            return Ok(());
        }
        let count = cm.count();
        if count <= MAX_FLEX_ID_PER_SHARD {
            let bytes = encode_map(&cm);
            if !shard_needs_split(count, bytes.len()) {
                self.put_leaf_bytes(table_id, &cr, count as u32, &bytes)?;
                out.push(cr);
                return Ok(());
            }
        }
        // 1 段だけ覗いて、退化分割か実分割かを決める。
        let ((clo_r, clo), (chi_r, chi)) = cm.split_shard(cr).ok_or_else(|| {
            AppError::Internal("shard region cannot be split further".to_string())
        })?;
        if clo.is_empty() || chi.is_empty() {
            // 退化分割：中間ポインタを作らず孫を巻き上げる。
            self.emit_child(table_id, clo_r, clo, out)?;
            self.emit_child(table_id, chi_r, chi, out)?;
        } else {
            let mut grand = Vec::new();
            self.emit_child(table_id, clo_r, clo, &mut grand)?;
            self.emit_child(table_id, chi_r, chi, &mut grand)?;
            self.db.tables_data.put(
                &mut self.write_txn,
                &(table_id, cr),
                &ShardEntry::encode_pointers(&grand),
            )?;
            out.push(cr);
        }
        Ok(())
    }

    fn put_leaf(
        &mut self,
        table_id: TableId,
        region: &FlexId,
        map: &SpatialIdMap<Vec<u8>>,
    ) -> Result<(), AppError> {
        let bytes = encode_map(map);
        self.put_leaf_bytes(table_id, region, map.count() as u32, &bytes)
    }

    /// 呼び出し元で分割要否の判定用に既に直列化済みのバイト列を、二重に直列化せず保存する。
    fn put_leaf_bytes(
        &mut self,
        table_id: TableId,
        region: &FlexId,
        count: u32,
        bytes: &[u8],
    ) -> Result<(), AppError> {
        self.db.tables_data.put(
            &mut self.write_txn,
            &(table_id, *region),
            &ShardEntry::encode_leaf(count, bytes),
        )?;
        Ok(())
    }
}
