//! ツリーの形と分割・統合の規則は TiKV 実装と同一で、違うのは取得手段だけ。

use rustc_hash::{FxHashMap, FxHashSet};

use kasane_logic::{FlexId, SpatialIdMap, SpatialIdSet};

use crate::error::AppError;
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::repositories::ValueGroups;
use crate::repositories::encoding::shard_entry::{
    MAX_SHARD_BYTES, MERGE_FLEX_ID_THRESHOLD, ShardEntry, shard_needs_split,
};
use crate::repositories::encoding::value_index;

use super::shard;
use super::{KasaneDbRead, KasaneDbWrite};

type ValueMap = FxHashMap<Vec<u8>, Vec<FlexId>>;

/// rayon へ出す基準（触れる**リーフ数**）。
///
/// クエリ FlexId 数で判定すると、広域検索（FlexId が数個でも数千の葉に及ぶ）が単一
/// スレッドへ落ちる。
const DATA_GET_LEAF_PARALLEL_THRESHOLD: usize = 32;

impl<'a> KasaneDbRead<'a> {
    /// 指定範囲の空間 ID を値ごとにグループ化して返す。
    ///
    /// リーフ単位で並列化できるのは、同一 FlexId が 1 つの葉にしか属さないため。
    #[tracing::instrument(skip_all)]
    pub fn data_get_impl(
        &self,
        table_id: crate::models::id::TableId,
        ids: SpatialIdSet,
        limit: Option<usize>,
    ) -> Result<ValueGroups, AppError> {
        let mut flex_ids_iter = ids.flex_ids();
        let counter = std::sync::atomic::AtomicUsize::new(0);
        let mut global_by_value = ValueMap::default();

        // 数百万件の全件ルーティングを踏ませないよう、打ち切れる粒度に区切る。
        const ROUTING_BATCH_SIZE: usize = 65536;

        loop {
            if let Some(limit) = limit
                && counter.load(std::sync::atomic::Ordering::Relaxed) >= limit
            {
                break;
            }

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

            let by_leaf = shard::route_leaves_batched(
                &self.db.tables_data,
                &self.read_txn,
                table_id,
                batch.iter(),
            )?;
            let parallel = by_leaf.len() >= DATA_GET_LEAF_PARALLEL_THRESHOLD;

            if !parallel {
                for (region, queries) in by_leaf {
                    if let Some(limit) = limit
                        && counter.load(std::sync::atomic::Ordering::Relaxed) >= limit
                    {
                        break;
                    }
                    Self::resolve_leaf(
                        &self.db.tables_data,
                        &self.read_txn,
                        table_id,
                        &region,
                        &queries,
                        &mut global_by_value,
                        limit,
                        Some(&counter),
                    )?;
                }
            } else {
                use rayon::prelude::*;
                let tables_data = self.db.tables_data;
                let env = &self.db.env;

                // チャンクごとに 1 txn。葉ごとに開くと開設コストが葉数に比例する。
                let entries: Vec<(FlexId, Vec<FlexId>)> = by_leaf.into_iter().collect();
                let chunk_size = entries
                    .len()
                    .div_ceil(rayon::current_num_threads().max(1))
                    .max(1);
                let partials: Vec<ValueMap> = entries
                    .par_chunks(chunk_size)
                    .map(|chunk| -> Result<ValueMap, AppError> {
                        let txn = env
                            .read_txn()
                            .map_err(|e| AppError::InternalError(e.to_string()))?;
                        let mut local = ValueMap::default();
                        for (region, queries) in chunk {
                            if let Some(limit) = limit
                                && counter.load(std::sync::atomic::Ordering::Relaxed) >= limit
                            {
                                break;
                            }
                            Self::resolve_leaf(
                                &tables_data,
                                &txn,
                                table_id,
                                region,
                                queries,
                                &mut local,
                                limit,
                                Some(&counter),
                            )?;
                        }
                        Ok(local)
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                // 部分マップを値でマージ。
                for partial in partials {
                    for (value, mut flex_ids) in partial {
                        global_by_value
                            .entry(value)
                            .or_default()
                            .append(&mut flex_ids);
                    }
                }
            }
        }

        Ok(global_by_value.into_iter().collect())
    }

    /// 1 枚のリーフを走査し、`by_value`（値 → FlexId 群）へ積む。
    ///
    /// 葉ローカルの辞書インデックス（`u32`）で先にグルーピングするのは、値バイト列で直接
    /// ハッシュすると結果 FlexId の数だけ長いバイト列をハッシュすることになるため。
    #[allow(clippy::too_many_arguments)]
    fn resolve_leaf(
        tables_data: &heed::Database<super::keys::TableIdAndFlexId, heed::types::Bytes>,
        txn: &heed::RoTxn<heed::WithoutTls>,
        table_id: TableId,
        region: &FlexId,
        queries: &[FlexId],
        by_value: &mut ValueMap,
        limit: Option<usize>,
        counter: Option<&std::sync::atomic::AtomicUsize>,
    ) -> Result<(), AppError> {
        let Some(arch) = shard::load_leaf_archived(tables_data, txn, table_id, region)? else {
            return Ok(());
        };

        let mut local_count = 0;
        let batch_size = limit.unwrap_or(0).clamp(1, 256);

        let mut local: FxHashMap<u32, Vec<FlexId>> = FxHashMap::default();
        for query_flex in queries {
            if let (Some(limit), Some(counter)) = (limit, counter)
                && counter.load(std::sync::atomic::Ordering::Relaxed) >= limit
            {
                break;
            }
            arch.get_indexed(query_flex, |got_flex, packed| {
                if let (Some(limit), Some(counter)) = (limit, counter) {
                    if counter.load(std::sync::atomic::Ordering::Relaxed) >= limit {
                        return;
                    }
                    local_count += 1;
                    if local_count >= batch_size {
                        counter.fetch_add(local_count, std::sync::atomic::Ordering::Relaxed);
                        local_count = 0;
                    }
                }
                local.entry(packed).or_default().push(got_flex);
            });
        }

        if let (Some(_limit), Some(counter)) = (limit, counter)
            && local_count > 0
        {
            counter.fetch_add(local_count, std::sync::atomic::Ordering::Relaxed);
        }

        // 葉に現れた distinct 値の数だけ実バイト列へ復元する。
        for (packed, mut flex_ids) in local {
            let value = arch.value_bytes(packed);
            if let Some(existing) = by_value.get_mut(value) {
                existing.append(&mut flex_ids);
            } else {
                by_value.insert(value.to_vec(), flex_ids);
            }
        }
        Ok(())
    }
}

impl<'a> KasaneDbRead<'a> {
    /// 結果を `Vec` で返すのは、遅延イテレータを外へ持ち出せない TiKV と署名を揃えるため。
    #[tracing::instrument(skip_all)]
    pub fn data_filter_eq_impl(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        value: &[u8],
    ) -> Result<Vec<FlexId>, AppError> {
        let prefix =
            value_index::make_prefix(table_id, &value_index::order_preserving(data_type, value));

        let mut out = Vec::new();
        for item in self
            .db
            .value_index
            .prefix_iter(&self.read_txn, prefix.as_slice())?
        {
            let (key, _) = item?;
            // 可変長値で前方一致しただけの別キーを除外（残りがちょうど flexid 分の長さ）。
            if key.len() != prefix.len() + FlexId::ENCODED_LEN {
                continue;
            }
            out.push(value_index::flexid_from_key(key)?);
        }
        Ok(out)
    }

    /// 値が `lo`〜`hi`（両端含む）に入る FlexId を引く。
    ///
    /// 可変長型はバイト範囲だけで絞りきれないので、型の幅で読む範囲を決めてから取り出した
    /// `vkey` で厳密に絞る（[`value_index::range_scan_bounds`]）。
    #[tracing::instrument(skip_all)]
    pub fn data_filter_range_impl(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        lo: &[u8],
        hi: &[u8],
    ) -> Result<Vec<FlexId>, AppError> {
        let lo_vkey = value_index::order_preserving(data_type, lo);
        let hi_vkey = value_index::order_preserving(data_type, hi);

        let mut out = Vec::new();
        let keep = |key: &[u8], out: &mut Vec<FlexId>| -> Result<(), AppError> {
            let vkey = value_index::vkey_from_key(key)?;
            if vkey < lo_vkey.as_slice() || vkey > hi_vkey.as_slice() {
                return Ok(());
            }
            out.push(value_index::flexid_from_key(key)?);
            Ok(())
        };

        if data_type.has_fixed_width_value() {
            // 全キーが同じ長さなので、この範囲が過不足なく該当行を覆う。
            let start = value_index::make_prefix(table_id, &lo_vkey);
            // hi 側は flexid 部を最大化して `(hi, *)` まで含める。
            let mut end = value_index::make_prefix(table_id, &hi_vkey);
            end.extend_from_slice(&[0xFF; FlexId::ENCODED_LEN]);
            let bounds = (
                std::ops::Bound::Included(start.as_slice()),
                std::ops::Bound::Included(end.as_slice()),
            );
            for item in self.db.value_index.range(&self.read_txn, &bounds)? {
                let (key, _) = item?;
                keep(key, &mut out)?;
            }
        } else {
            // 覆う最小の範囲まで絞り、あとは `keep` の厳密判定に任せる。
            let (start, end) = value_index::range_scan_bounds(table_id, &lo_vkey, &hi_vkey);
            let bounds = (
                std::ops::Bound::Included(start.as_slice()),
                match &end {
                    Some(end) => std::ops::Bound::Excluded(end.as_slice()),
                    None => std::ops::Bound::Unbounded,
                },
            );
            for item in self.db.value_index.range(&self.read_txn, &bounds)? {
                let (key, _) = item?;
                keep(key, &mut out)?;
            }
        }
        Ok(out)
    }
}

impl<'a> KasaneDbWrite<'a> {
    #[tracing::instrument(skip_all)]
    pub fn data_insert_impl(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let by_leaf = self.group_by_leaf(table_id, ids.flex_ids())?;
        for (region, flex_ids) in by_leaf {
            let map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            self.apply_leaf(table_id, index, region, map, &flex_ids, |m| {
                for flex_id in &flex_ids {
                    m.insert(*flex_id, data.to_vec());
                }
            })?;
        }
        Ok(())
    }

    /// まだ値が無い空間 ID にだけ書く（既存値は保持）。
    #[tracing::instrument(skip_all)]
    pub fn data_upsert_impl(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let by_leaf = self.group_by_leaf(table_id, ids.flex_ids())?;
        let data_vec = data.to_vec();
        for (region, flex_ids) in by_leaf {
            let map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            self.apply_leaf(table_id, index, region, map, &flex_ids, |m| {
                let mut target_set = SpatialIdSet::new();
                for flex_id in &flex_ids {
                    let occupied_set: SpatialIdSet = m.get(flex_id).map(|(f, _)| f).collect();
                    target_set.clear();
                    target_set.insert(*flex_id);

                    for f in (&target_set - &occupied_set).flex_ids() {
                        m.insert(f, data_vec.clone());
                    }
                }
            })?;
        }
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn data_remove_impl(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        let by_leaf = self.group_by_leaf(table_id, ids.flex_ids())?;
        let mut affected: Vec<FlexId> = Vec::new();
        for (region, flex_ids) in by_leaf {
            let map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            self.apply_leaf(table_id, index, region, map, &flex_ids, |m| {
                for flex_id in &flex_ids {
                    m.remove(flex_id);
                }
            })?;
            affected.push(region);
        }
        for region in affected {
            self.try_merge_up(table_id, index, region)?;
        }
        Ok(())
    }

    /// 削除でデータ量が減ったリーフを親へ統合し、可能な限り木を圧縮する。
    fn try_merge_up(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        region: FlexId,
    ) -> Result<(), AppError> {
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

            // 統合は木の形を変えるだけで `(FlexId, 値)` の対応は変わらない。
            let old_keys = index.map(|data_type| {
                let mut keys = FxHashSet::default();
                for m in &child_maps {
                    for (f, v) in m.iter() {
                        keys.insert(value_index::make_key(
                            table_id,
                            &value_index::order_preserving(data_type, v),
                            &f,
                        ));
                    }
                }
                keys
            });

            let merged = SpatialIdMap::merge_shards(parent_region, child_maps)?;

            if let (Some(data_type), Some(old_keys)) = (index, old_keys) {
                let mut new_keys = FxHashSet::default();
                for (f, v) in merged.iter() {
                    new_keys.insert(value_index::make_key(
                        table_id,
                        &value_index::order_preserving(data_type, v),
                        &f,
                    ));
                }
                self.update_value_index(old_keys, new_keys)?;
            }

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

    fn apply_leaf<F>(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        region: FlexId,
        mut map: SpatialIdMap<Vec<u8>>,
        input: &[FlexId],
        modify: F,
    ) -> Result<(), AppError>
    where
        F: FnOnce(&mut SpatialIdMap<Vec<u8>>),
    {
        // 索引キーは格納 `FlexId` 1 件につき 1 つ増えるので、ここで 3 桁変わる。
        let Some(data_type) = index else {
            modify(&mut map);
            return self.store_shard(table_id, region, map);
        };

        let scan: SpatialIdSet = input.iter().cloned().collect();

        let mut old_keys = FxHashSet::default();
        let mut pre_modify_scan = scan.clone();
        for f_scan in scan.iter() {
            for (f, v) in map.get_overlapping(&f_scan) {
                old_keys.insert(value_index::make_key(
                    table_id,
                    &value_index::order_preserving(data_type, v),
                    &f,
                ));
                pre_modify_scan.insert(f);
            }
        }

        modify(&mut map);

        let mut new_keys = FxHashSet::default();
        for f_scan in pre_modify_scan.iter() {
            for (f, v) in map.get_overlapping(&f_scan) {
                new_keys.insert(value_index::make_key(
                    table_id,
                    &value_index::order_preserving(data_type, v),
                    &f,
                ));
            }
        }

        self.update_value_index(old_keys, new_keys)?;

        self.store_shard(table_id, region, map)?;

        Ok(())
    }

    /// 昇順に並べてから適用するのは、B-tree ではランダム順よりキャッシュ効率が高いため。
    fn update_value_index(
        &mut self,
        old_keys: FxHashSet<Vec<u8>>,
        new_keys: FxHashSet<Vec<u8>>,
    ) -> Result<(), AppError> {
        let mut to_delete: Vec<&Vec<u8>> = old_keys.difference(&new_keys).collect();
        to_delete.sort_unstable();
        for key in to_delete {
            self.db.value_index.delete(&mut self.write_txn, key)?;
        }

        let mut to_put: Vec<&Vec<u8>> = new_keys.difference(&old_keys).collect();
        to_put.sort_unstable();
        for key in to_put {
            self.db.value_index.put(&mut self.write_txn, key, &())?;
        }
        Ok(())
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

        let bytes = map
            .to_bytes()
            .map_err(|e| AppError::InternalError(format!("rkyv serialize: {e}")))?;

        if !shard_needs_split(map.count(), bytes.len()) {
            self.put_leaf_bytes(table_id, &region, map.count() as u32, &bytes)?;
            return Ok(());
        }

        // 分割が必要 → パス圧縮した被覆子領域を構築し、親をポインタノードにする。
        let mut children = Vec::new();
        self.cover_split(table_id, &map, &mut children)?;
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
        map: &SpatialIdMap<Vec<u8>>,
        out: &mut Vec<FlexId>,
    ) -> Result<(), AppError> {
        let ((lo_r, lo), (hi_r, hi)) = map
            .split_shard()
            .ok_or_else(|| AppError::InternalError("split on shardless map".to_string()))?;
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
        let bytes = cm
            .to_bytes()
            .map_err(|e| AppError::InternalError(format!("rkyv serialize: {e}")))?;
        if !shard_needs_split(cm.count(), bytes.len()) {
            self.put_leaf_bytes(table_id, &cr, cm.count() as u32, &bytes)?;
            out.push(cr);
            return Ok(());
        }
        // 1 段だけ覗いて、退化分割か実分割かを決める。
        let ((clo_r, clo), (chi_r, chi)) = cm
            .split_shard()
            .ok_or_else(|| AppError::InternalError("split on shardless map".to_string()))?;
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

    /// 件数ヘッダ付きで保存する。
    fn put_leaf(
        &mut self,
        table_id: TableId,
        region: &FlexId,
        map: &SpatialIdMap<Vec<u8>>,
    ) -> Result<(), AppError> {
        let bytes = map
            .to_bytes()
            .map_err(|e| AppError::InternalError(format!("rkyv serialize: {e}")))?;
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
