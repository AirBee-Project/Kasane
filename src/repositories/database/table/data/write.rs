use std::collections::{HashMap, HashSet};

use kasane_logic::{FlexId, IntoFlexIds, SpatialIdMap, SpatialIdSet};

use super::shard::{self, MAX_FLEX_ID_PER_SHARD, MERGE_FLEX_ID_THRESHOLD, ShardEntry};
use super::value_index;
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::{error::AppError, repositories::KasaneDbWrite};

impl<'a> KasaneDbWrite<'a> {
    /// 空間IDにデータを書き込む。全て上書き。
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

    /// 空間IDにデータを書き込む。値が既に存在する場合は無視。
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

    /// 空間IDに紐づく値を削除する。
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
        for region in affected {
            self.try_merge_up(table_id, data_type, region)?;
        }
        Ok(())
    }

    /// `region` から親へ遡り、親ポインタノードの子（兄弟）合算が [`MERGE_FLEX_ID_THRESHOLD`] 以下なら1つのリーフへ統合する。可能な限り上へ伝播する。
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

            // パス圧縮により子は可変数（2 以上）。子を走査し、いずれかがポインタノードなら
            // このレベルは統合しない。全リーフ（＋空マーカ）で合算が閾値以下なら1リーフへ畳み込む。
            let mut child_maps: Vec<SpatialIdMap<Vec<u8>>> = Vec::new();
            let mut combined = 0usize;
            let mut mergeable = true;
            for cr in &child_regions {
                // 空領域のKeyはそもそも存在しないのでスキップ
                let Some(bytes) = self
                    .db
                    .tables_data
                    .get(&self.write_txn, &(table_id, cr.clone()))?
                else {
                    continue;
                };
                match ShardEntry::decode(bytes)? {
                    ShardEntry::Leaf(map_bytes) => {
                        let m = unsafe { SpatialIdMap::<Vec<u8>>::from_bytes(&map_bytes) }
                            .map_err(|e| {
                                AppError::InternalError(format!("rkyv deserialize: {e}"))
                            })?;
                        combined += m.count();
                        if combined > MERGE_FLEX_ID_THRESHOLD {
                            mergeable = false;
                            break;
                        }
                        child_maps.push(m);
                    }
                    ShardEntry::Pointers(_) => {
                        mergeable = false;
                        break;
                    }
                }
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

            // 子リーフ群を親領域へ畳み込む（union。境界跨ぎの同値は compaction される）。
            let merged = SpatialIdMap::merge_shards(parent_region.clone(), child_maps)?;

            let mut new_keys = HashSet::new();
            for (f, v) in merged.iter() {
                new_keys.insert(value_index::make_key(
                    table_id,
                    &value_index::order_preserving(data_type, v),
                    &f,
                ));
            }

            // 値インデックス差分更新
            self.update_value_index(old_keys, new_keys)?;

            // 親キーをリーフ（空なら削除）に置換し、子キーを削除。
            let parent_key = (table_id, parent_region.clone());
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
                    .delete(&mut self.write_txn, &(table_id, cr.clone()))?;
            }

            // 親が新たなリーフになった → さらに上へ伝播。
            region = parent_region;
        }
        Ok(())
    }

    /// 1 リーフへの変更を適用し、値インデックスを差分更新したうえで保存する。
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

        // 保存（過大なら2分割、空なら削除）。
        self.store_shard(table_id, region, map)?;

        Ok(())
    }

    /// 旧/新のインデックスキーセットの対称差だけ `value_index` を更新する。
    ///
    /// 削除・追加とも**キー昇順**に適用する。`value_index` は LMDB（B-tree）なので、
    /// ソート済みの逐次アクセスはランダム順より挿入局所性が良くキャッシュ効率が高い。
    fn update_value_index(
        &mut self,
        old_keys: HashSet<Vec<u8>>,
        new_keys: HashSet<Vec<u8>>,
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

    /// 与えた FlexId 群を、ポインタツリーを**一度の降下**で辿って担当リーフ領域ごとにまとめる。
    fn group_by_leaf(
        &self,
        table_id: TableId,
        flex_ids: impl Iterator<Item = FlexId>,
    ) -> Result<HashMap<FlexId, Vec<FlexId>>, AppError> {
        let ids: Vec<FlexId> = flex_ids.collect();
        shard::route_leaves_batched(&self.db.tables_data, &self.write_txn, table_id, &ids)
    }

    /// シャードを保存する。閾値超過なら**データが実際に割れる軸まで分割を畳み込み**（適応軸＝パス圧縮）、
    /// 親キーを**被覆ポインタノード**にする。
    ///
    /// 内部は正準軸（F→X→Y, level%3）での二分割を繰り返すが、片側が空になる**退化分割は
    /// ポインタノードを作らず子を上位へ巻き上げる**。その結果 materialize されるポインタノードは
    /// XY のようにデータが実際に分割される軸で枝分かれし、F に偏りがなければ F の退化ノードは生成
    /// されない（FlexId の独立軸ズームを活かす）。被覆は常に保たれる（空領域はキーを持たないが
    /// 親ポインタが領域を列挙するためルーティングで取りこぼされない）。空マップはキーを削除する。
    ///
    /// 前提: `region` は呼び出し時点で**リーフ（または未作成）**。よって分割はこの場で初めて
    /// 部分木キーを生成し、孤児キーは生じない。
    fn store_shard(
        &mut self,
        table_id: TableId,
        region: FlexId,
        map: SpatialIdMap<Vec<u8>>,
    ) -> Result<(), AppError> {
        let key = (table_id, region.clone());

        if !map.should_split_shard(MAX_FLEX_ID_PER_SHARD) {
            if map.is_empty() {
                self.db.tables_data.delete(&mut self.write_txn, &key)?;
            } else {
                self.put_leaf(table_id, &region, &map)?;
            }
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

    /// 分割が必要な `map` を正準二分割し、各子を [`emit_child`](Self::emit_child) で処理して
    /// **被覆子領域**を `out` に積む。
    fn cover_split(
        &mut self,
        table_id: TableId,
        map: &SpatialIdMap<Vec<u8>>,
        out: &mut Vec<FlexId>,
    ) -> Result<(), AppError> {
        // should_split が真 ⇒ シャード領域があり split_shard は Some。
        let ((lo_r, lo), (hi_r, hi)) = map
            .split_shard()
            .ok_or_else(|| AppError::InternalError("split on shardless map".to_string()))?;
        self.emit_child(table_id, lo_r, lo, out)?;
        self.emit_child(table_id, hi_r, hi, out)?;
        Ok(())
    }

    /// 子シャード `(cr, cm)` を被覆集合 `out` に組み込む（パス圧縮の本体）。
    /// - 空 → 領域だけ被覆として積む（キーは持たない）。
    /// - 収まる → リーフとして保存し領域を積む。
    /// - 過大かつ片側が空になる退化分割 → ポインタノードを作らず**孫を巻き上げ**る。
    /// - 過大で両側に割れる → `cr` を独立ポインタノードとして保存し領域を積む。
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
                .delete(&mut self.write_txn, &(table_id, cr.clone()))?;
            out.push(cr);
            return Ok(());
        }
        if !cm.should_split_shard(MAX_FLEX_ID_PER_SHARD) {
            self.put_leaf(table_id, &cr, &cm)?;
            out.push(cr);
            return Ok(());
        }
        // 過大：1段だけ覗いて、圧縮（退化）か枝分かれ（実分割）かを決める。
        let ((clo_r, clo), (chi_r, chi)) = cm
            .split_shard()
            .ok_or_else(|| AppError::InternalError("split on shardless map".to_string()))?;
        if clo.is_empty() || chi.is_empty() {
            // 退化分割：cr にポインタノードを作らず孫を out へ巻き上げる（チェーン圧縮）。
            self.emit_child(table_id, clo_r, clo, out)?;
            self.emit_child(table_id, chi_r, chi, out)?;
        } else {
            // 実分割：cr を独立ポインタノードにする。
            let mut grand = Vec::new();
            self.emit_child(table_id, clo_r, clo, &mut grand)?;
            self.emit_child(table_id, chi_r, chi, &mut grand)?;
            self.db.tables_data.put(
                &mut self.write_txn,
                &(table_id, cr.clone()),
                &ShardEntry::encode_pointers(&grand),
            )?;
            out.push(cr);
        }
        Ok(())
    }

    /// リーフを保持 [`FlexId`] 件数ヘッダ付きで保存する。
    fn put_leaf(
        &mut self,
        table_id: TableId,
        region: &FlexId,
        map: &SpatialIdMap<Vec<u8>>,
    ) -> Result<(), AppError> {
        let bytes = map
            .to_bytes()
            .map_err(|e| AppError::InternalError(format!("rkyv serialize: {e}")))?;
        self.db.tables_data.put(
            &mut self.write_txn,
            &(table_id, region.clone()),
            &ShardEntry::encode_leaf(map.count() as u32, &bytes),
        )?;
        Ok(())
    }
}
