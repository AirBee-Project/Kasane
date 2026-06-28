use std::collections::{HashMap, HashSet};

use kasane_logic::{FlexId, IntoFlexIds, SpatialIdMap, SpatialIdSet};

use super::shard::{self, MAX_FLEX_ID_PER_SHARD, ShardEntry};
use super::value_index;
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::{error::AppError, repositories::KasaneDbWrite};

impl<'a> KasaneDbWrite<'a> {
    /// リポジトリ層にデータを挿入する（既存値があっても上書き）。
    ///
    /// 入力は正規化済みの [`SpatialIdSet`]。`into_flex_ids()` の**圧縮 FlexId**で操作するため、
    /// 粗い領域でも単体セルへ展開せず効率的に挿入できる。競合検証はシャード内
    /// （[`SpatialIdMap`]）で行われる。
    pub fn data_insert(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let by_leaf = self.group_by_leaf(table_id, ids.into_flex_ids())?;
        let mut delta: i64 = 0;
        for (region, flex_ids) in by_leaf {
            let map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            delta += self.apply_leaf(table_id, data_type, region, map, &flex_ids, |m| {
                for flex_id in &flex_ids {
                    m.insert(flex_id.clone(), data.to_vec());
                }
            })?;
        }
        self.adjust_table_count(table_id, delta)?;
        Ok(())
    }

    /// 値が存在しないセルにのみ書き込む（Upsert）。
    pub fn data_upsert(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let by_leaf = self.group_by_leaf(table_id, ids.into_flex_ids())?;
        let mut delta: i64 = 0;
        for (region, flex_ids) in by_leaf {
            let map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            delta += self.apply_leaf(table_id, data_type, region, map, &flex_ids, |m| {
                for flex_id in &flex_ids {
                    // 既に値があるセルは除外し、`target - 既存` のみ挿入する。
                    let occupied: Vec<FlexId> = m.get(flex_id).map(|(f, _)| f).collect();
                    let mut target_set = SpatialIdSet::new();
                    target_set.insert(flex_id.clone());
                    let mut occupied_set = SpatialIdSet::new();
                    for f in &occupied {
                        occupied_set.insert(f.clone());
                    }
                    for f in (&target_set - &occupied_set).into_flex_ids() {
                        m.insert(f, data.to_vec());
                    }
                }
            })?;
        }
        self.adjust_table_count(table_id, delta)?;
        Ok(())
    }

    /// 値を削除する。
    pub fn data_remove(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        let by_leaf = self.group_by_leaf(table_id, ids.into_flex_ids())?;
        let mut delta: i64 = 0;
        for (region, flex_ids) in by_leaf {
            let map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            delta += self.apply_leaf(table_id, data_type, region, map, &flex_ids, |m| {
                for flex_id in &flex_ids {
                    m.remove(flex_id).for_each(drop);
                }
            })?;
        }
        self.adjust_table_count(table_id, delta)?;
        Ok(())
    }

    /// 1 リーフへの変更を適用し、値インデックスを差分更新したうえで保存する。
    ///
    /// `modify` 実行前後で「入力 ∪ 旧リーフ領域」に重なるリーフ集合 (FlexId, 値) を取り、
    /// その対称差だけ `value_index` を put/delete する（分割で生じた残りリーフも拾える）。
    /// 戻り値は保持件数の増減（`table_counts` 用）。
    fn apply_leaf<F>(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        region: FlexId,
        mut map: SpatialIdMap<Vec<u8>>,
        input: &[FlexId],
        modify: F,
    ) -> Result<i64, AppError>
    where
        F: FnOnce(&mut SpatialIdMap<Vec<u8>>),
    {
        let mut scan = SpatialIdSet::new();
        for f in input {
            scan.insert(f.clone());
        }

        // 変更前の重なりリーフ（切り取らず）。
        let old: Vec<(FlexId, Vec<u8>)> = map
            .get_overlapping(&scan)
            .map(|(f, v)| (f, v.clone()))
            .collect();

        let before = map.count() as i64;
        modify(&mut map);
        let after = map.count() as i64;

        // 再スキャン範囲 = 入力 ∪ 旧リーフ領域（分割で生じた残りリーフも拾う）。
        for (f, _) in &old {
            scan.insert(f.clone());
        }
        let new: Vec<(FlexId, Vec<u8>)> = map
            .get_overlapping(&scan)
            .map(|(f, v)| (f, v.clone()))
            .collect();

        self.update_value_index(table_id, data_type, &old, &new)?;

        let key = (table_id, region.clone());
        if map.is_empty() {
            // 空になったリーフは削除（親ポインタは None 解決でリーフ扱いされるため安全）。
            self.db.tables_data.delete(&mut self.write_txn, &key)?;
        } else {
            self.store_leaf_with_split(table_id, region, map)?;
        }

        Ok(after - before)
    }

    /// 旧/新リーフ集合の対称差だけ `value_index` を更新する。
    fn update_value_index(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        old: &[(FlexId, Vec<u8>)],
        new: &[(FlexId, Vec<u8>)],
    ) -> Result<(), AppError> {
        let old_set: HashSet<(&FlexId, &Vec<u8>)> = old.iter().map(|(f, v)| (f, v)).collect();
        let new_set: HashSet<(&FlexId, &Vec<u8>)> = new.iter().map(|(f, v)| (f, v)).collect();

        for (f, v) in old {
            if !new_set.contains(&(f, v)) {
                let key = value_index::make_key(
                    table_id,
                    &value_index::order_preserving(data_type, v),
                    f,
                );
                self.db.value_index.delete(&mut self.write_txn, &key)?;
            }
        }
        for (f, v) in new {
            if !old_set.contains(&(f, v)) {
                let key = value_index::make_key(
                    table_id,
                    &value_index::order_preserving(data_type, v),
                    f,
                );
                self.db.value_index.put(&mut self.write_txn, &key, &())?;
            }
        }
        Ok(())
    }

    /// テーブルの保持件数カウンタ（`table_counts`）を `delta` だけ増減する。
    fn adjust_table_count(&mut self, table_id: TableId, delta: i64) -> Result<(), AppError> {
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
        table_id: TableId,
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
        table_id: TableId,
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
