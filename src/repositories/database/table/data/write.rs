use std::collections::{HashMap, HashSet};

use kasane_logic::{FlexId, IntoFlexIds, SpatialIdMap, SpatialIdSet};

use super::shard::{self, MAX_FLEX_ID_PER_SHARD, MERGE_FLEX_ID_THRESHOLD, ShardEntry};
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
        for (region, flex_ids) in by_leaf {
            let map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            self.apply_leaf(table_id, data_type, region, map, &flex_ids, |m| {
                for flex_id in &flex_ids {
                    m.insert(flex_id.clone(), data.to_vec());
                }
            })?;
        }
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
        for (region, flex_ids) in by_leaf {
            let map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            self.apply_leaf(table_id, data_type, region, map, &flex_ids, |m| {
                for flex_id in &flex_ids {
                    // 既に値があるセルは除外し、`target - 既存` のみ挿入する。
                    let occupied_set: SpatialIdSet = m.get(flex_id).map(|(f, _)| f).collect();
                    let mut target_set = SpatialIdSet::new();
                    target_set.insert(flex_id.clone());

                    for f in (&target_set - &occupied_set).into_flex_ids() {
                        m.insert(f, data.to_vec());
                    }
                }
            })?;
        }
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
        let mut affected: Vec<FlexId> = Vec::new();
        for (region, flex_ids) in by_leaf {
            let map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            self.apply_leaf(table_id, data_type, region.clone(), map, &flex_ids, |m| {
                for flex_id in &flex_ids {
                    m.remove(flex_id).for_each(drop);
                }
            })?;
            affected.push(region);
        }

        // remove で縮んだリーフは、兄弟と統合できるなら統合する（split の逆）。
        for region in affected {
            self.try_merge_up(table_id, data_type, region)?;
        }
        Ok(())
    }

    /// `region` から親へ遡り、親ポインタノードの子（兄弟）合算が
    /// [`MERGE_FLEX_ID_THRESHOLD`] 以下なら1つのリーフへ統合する。可能な限り上へ伝播する。
    fn try_merge_up(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        region: FlexId,
    ) -> Result<(), AppError> {
        let mut region = region;
        loop {
            let Some((parent_region, child_regions)) = shard::find_parent_pointer(
                &self.db.tables_data,
                &self.write_txn,
                table_id,
                &region,
            )?
            else {
                break;
            };

            // 子を順にロード。すべてリーフで、合算が閾値以下なら統合する。
            let mut child_maps: Vec<SpatialIdMap<Vec<u8>>> =
                Vec::with_capacity(child_regions.len());
            let mut combined = 0usize;
            let mut mergeable = true;
            for cr in &child_regions {
                let m = match self
                    .db
                    .tables_data
                    .get(&self.write_txn, &(table_id, cr.clone()))?
                {
                    Some(bytes) => match ShardEntry::decode(bytes)? {
                        ShardEntry::Leaf(map_bytes) => {
                            unsafe { SpatialIdMap::<Vec<u8>>::from_bytes(&map_bytes) }.map_err(
                                |e| AppError::InternalError(format!("rkyv deserialize: {e}")),
                            )?
                        }
                        // 子がまだ分割木 → このレベルでは統合しない。
                        ShardEntry::Pointers(_) => {
                            mergeable = false;
                            break;
                        }
                    },
                    None => SpatialIdMap::new_in_shard(cr.clone()),
                };
                combined += m.count();
                if combined > MERGE_FLEX_ID_THRESHOLD {
                    mergeable = false;
                    break;
                }
                child_maps.push(m);
            }
            if !mergeable {
                break;
            }

            // 値インデックス用に、変更前の全子リーフのキーを集める。
            let mut old_keys = HashSet::new();
            for m in &child_maps {
                for (f, v) in m.iter() {
                    old_keys.insert(value_index::make_key(
                        table_id,
                        &value_index::order_preserving(data_type, v),
                        &f,
                    ));
                }
            }

            // 統合（union。境界跨ぎの同値は compaction される）。
            let merged = SpatialIdMap::merge_siblings(parent_region.clone(), child_maps);

            let mut new_keys = HashSet::new();
            for (f, v) in merged.iter() {
                new_keys.insert(value_index::make_key(
                    table_id,
                    &value_index::order_preserving(data_type, v),
                    &f,
                ));
            }

            // 値インデックス差分更新。
            self.update_value_index(old_keys, new_keys)?;

            // 親キーをリーフ（空なら削除）に置換し、子キーを削除。
            let parent_key = (table_id, parent_region.clone());
            if merged.is_empty() {
                self.db
                    .tables_data
                    .delete(&mut self.write_txn, &parent_key)?;
            } else {
                let bytes = merged
                    .to_bytes()
                    .map_err(|e| AppError::InternalError(format!("rkyv serialize: {e}")))?;
                self.db.tables_data.put(
                    &mut self.write_txn,
                    &parent_key,
                    &ShardEntry::encode_leaf(&bytes),
                )?;
            }
            for cr in &child_regions {
                self.db
                    .tables_data
                    .delete(&mut self.write_txn, &(table_id, cr.clone()))?;
            }

            // 親が新たなリーフになった → さらに上へ伝播。
            region = parent_region;
        }
        Ok(())
    }

    /// 1 リーフへの変更を適用し、値インデックスを差分更新したうえで保存する。
    ///
    /// `modify` 実行前後で「入力 ∪ 旧リーフ領域」に重なるリーフ集合 (FlexId, 値) を取り、
    /// その対称差だけ `value_index` を put/delete する（分割で生じた残りリーフも拾える）。
    fn apply_leaf<F>(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        region: FlexId,
        mut map: SpatialIdMap<Vec<u8>>,
        input: &[FlexId],
        modify: F,
    ) -> Result<(), AppError>
    where
        F: FnOnce(&mut SpatialIdMap<Vec<u8>>),
    {
        let mut scan: SpatialIdSet = input.iter().cloned().collect();

        // 変更前の重なりリーフからインデックスキーを計算
        let mut old_keys = HashSet::new();
        let mut old_flex_ids = Vec::new();
        for (f, v) in map.get_overlapping(&scan) {
            old_keys.insert(value_index::make_key(
                table_id,
                &value_index::order_preserving(data_type, v),
                &f,
            ));
            old_flex_ids.push(f);
        }

        modify(&mut map);

        // 再スキャン範囲 = 入力 ∪ 旧リーフ領域（分割で生じた残りリーフも拾う）。
        scan.extend(old_flex_ids);

        // 変更後の重なりリーフからインデックスキーを計算
        let mut new_keys = HashSet::new();
        for (f, v) in map.get_overlapping(&scan) {
            new_keys.insert(value_index::make_key(
                table_id,
                &value_index::order_preserving(data_type, v),
                &f,
            ));
        }

        self.update_value_index(old_keys, new_keys)?;

        let key = (table_id, region.clone());
        if map.is_empty() {
            // 空になったリーフは削除（親ポインタは None 解決でリーフ扱いされるため安全）。
            self.db.tables_data.delete(&mut self.write_txn, &key)?;
        } else {
            self.store_leaf_with_split(table_id, region, map)?;
        }

        Ok(())
    }

    /// 旧/新のインデックスキーセットの対称差だけ `value_index` を更新する。
    fn update_value_index(
        &mut self,
        old_keys: HashSet<Vec<u8>>,
        new_keys: HashSet<Vec<u8>>,
    ) -> Result<(), AppError> {
        for key in &old_keys {
            if !new_keys.contains(key) {
                self.db.value_index.delete(&mut self.write_txn, key)?;
            }
        }
        for key in &new_keys {
            if !old_keys.contains(key) {
                self.db.value_index.put(&mut self.write_txn, key, &())?;
            }
        }
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
