use std::collections::HashMap;

use kasane_logic::{FlexId, IntoFlexIds, SpatialId, SpatialIdMap, SpatialIdSet};

use super::shard::{self, MAX_FLEX_ID_PER_SHARD, ShardEntry};
use crate::{error::AppError, repositories::KasaneDbWrite};

impl<'a> KasaneDbWrite<'a> {
    /// リポジトリ層にデータを挿入する（既存値があっても上書き）。
    /// 競合検証はシャード内（[`SpatialIdMap`]）で行われる。
    pub fn data_insert<I: SpatialId>(
        &mut self,
        table_id: crate::models::id::TableId,
        ids: impl Iterator<Item = I>,
        data: &[u8],
    ) -> Result<(), AppError> {
        let by_leaf = self.group_by_leaf(table_id, ids.flat_map(|s| s.into_flex_ids()))?;
        let mut delta: i64 = 0;
        for (region, flex_ids) in by_leaf {
            let mut map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            let before = map.count() as i64;
            for flex_id in flex_ids {
                map.insert(flex_id, data.to_vec());
            }
            delta += map.count() as i64 - before;
            self.store_leaf_with_split(table_id, region, map)?;
        }
        self.adjust_table_count(table_id, delta)?;
        Ok(())
    }

    /// 値が存在しないセルにのみ書き込む（Upsert）。
    pub fn data_upsert(
        &mut self,
        table_id: crate::models::id::TableId,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let by_leaf = self.group_by_leaf(table_id, ids.into_flex_ids())?;
        let mut delta: i64 = 0;
        for (region, flex_ids) in by_leaf {
            let mut map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            let before = map.count() as i64;
            for flex_id in &flex_ids {
                // 既に値があるセルは除外し、`target - 既存` のみ挿入する。
                let occupied: Vec<FlexId> = map.get(flex_id).map(|(f, _)| f).collect();
                let mut target_set = SpatialIdSet::new();
                target_set.insert(flex_id.clone());
                let mut occupied_set = SpatialIdSet::new();
                for f in &occupied {
                    occupied_set.insert(f.clone());
                }
                for f in (&target_set - &occupied_set).into_flex_ids() {
                    map.insert(f, data.to_vec());
                }
            }
            delta += map.count() as i64 - before;
            self.store_leaf_with_split(table_id, region, map)?;
        }
        self.adjust_table_count(table_id, delta)?;
        Ok(())
    }

    /// 値を削除する。
    pub fn data_remove(
        &mut self,
        table_id: crate::models::id::TableId,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        let by_leaf = self.group_by_leaf(table_id, ids.into_flex_ids())?;
        let mut delta: i64 = 0;
        for (region, flex_ids) in by_leaf {
            let mut map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            let before = map.count() as i64;
            for flex_id in &flex_ids {
                map.remove(flex_id).for_each(drop);
            }
            delta += map.count() as i64 - before;

            let key = (table_id, region.clone());
            if map.is_empty() {
                // 空になったリーフは削除（親ポインタは None 解決でリーフ扱いされるため安全）。
                self.db.tables_data.delete(&mut self.write_txn, &key)?;
            } else {
                let bytes = map
                    .to_bytes()
                    .map_err(|e| AppError::InternalError(format!("rkyv serialize: {e}")))?;
                self.db.tables_data.put(
                    &mut self.write_txn,
                    &key,
                    &ShardEntry::encode_leaf(&bytes),
                )?;
            }
        }
        self.adjust_table_count(table_id, delta)?;
        Ok(())
    }

    /// テーブルの保持件数カウンタ（`table_counts`）を `delta` だけ増減する。
    fn adjust_table_count(
        &mut self,
        table_id: crate::models::id::TableId,
        delta: i64,
    ) -> Result<(), AppError> {
        if delta == 0 {
            return Ok(());
        }
        let current = self
            .db
            .table_counts
            .get(&self.write_txn, &table_id)?
            .unwrap_or(0);
        let next = (current as i64 + delta).max(0) as u64;
        self.db
            .table_counts
            .put(&mut self.write_txn, &table_id, &next)?;
        Ok(())
    }

    /// 与えた FlexId 群を、ポインタツリーを辿って担当リーフ領域ごとにまとめる。
    fn group_by_leaf(
        &self,
        table_id: crate::models::id::TableId,
        flex_ids: impl Iterator<Item = FlexId>,
    ) -> Result<HashMap<FlexId, Vec<FlexId>>, AppError> {
        let mut by_leaf: HashMap<FlexId, Vec<FlexId>> = HashMap::new();
        for flex_id in flex_ids {
            for leaf in
                shard::route_leaves(&self.db.tables_data, &self.write_txn, table_id, &flex_id)?
            {
                by_leaf.entry(leaf).or_default().push(flex_id.clone());
            }
        }
        Ok(by_leaf)
    }

    /// リーフを保存する。閾値超過なら動的分割し、親キーを子へのポインタノードに置換する。
    fn store_leaf_with_split(
        &mut self,
        table_id: crate::models::id::TableId,
        region: FlexId,
        map: SpatialIdMap<Vec<u8>>,
    ) -> Result<(), AppError> {
        let key = (table_id, region.clone());

        if map.should_split_shard(MAX_FLEX_ID_PER_SHARD) {
            let children = map.split_shard(MAX_FLEX_ID_PER_SHARD);
            if children.len() >= 2 {
                let mut child_regions = Vec::with_capacity(children.len());
                for child in &children {
                    let child_region = child.shard().cloned().ok_or_else(|| {
                        AppError::InternalError("split child has no shard region".to_string())
                    })?;
                    let child_bytes = child
                        .to_bytes()
                        .map_err(|e| AppError::InternalError(format!("rkyv serialize: {e}")))?;
                    self.db.tables_data.put(
                        &mut self.write_txn,
                        &(table_id, child_region.clone()),
                        &ShardEntry::encode_leaf(&child_bytes),
                    )?;
                    child_regions.push(child_region);
                }
                self.db.tables_data.put(
                    &mut self.write_txn,
                    &key,
                    &ShardEntry::encode_pointers(&child_regions),
                )?;
                return Ok(());
            }
            // 分割できなかった（1 ピース）場合はリーフ保存にフォールバック。
        }

        let bytes = map
            .to_bytes()
            .map_err(|e| AppError::InternalError(format!("rkyv serialize: {e}")))?;
        self.db
            .tables_data
            .put(&mut self.write_txn, &key, &ShardEntry::encode_leaf(&bytes))?;
        Ok(())
    }
}
