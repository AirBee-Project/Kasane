use rustc_hash::{FxHashMap, FxHashSet};

use kasane_logic::{FlexId, SpatialIdMap, SpatialIdSet};

use super::shard::{self, MAX_FLEX_ID_PER_SHARD, MERGE_FLEX_ID_THRESHOLD, ShardEntry};
use super::value_index;
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::{error::AppError, repositories::KasaneDbWrite};

impl<'a> KasaneDbWrite<'a> {
    /// 指定された空間IDセット（`ids`）すべてに対して `data` を書き込む。
    /// 既に値が存在するIDについては、新しい値で上書きする。
    #[tracing::instrument(skip_all)]
    pub fn data_insert(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let by_leaf = self.group_by_leaf(table_id, ids.flex_ids())?;
        for (region, flex_ids) in by_leaf {
            let map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            self.apply_leaf(table_id, data_type, region, map, &flex_ids, |m| {
                for flex_id in &flex_ids {
                    m.insert(*flex_id, data.to_vec());
                }
            })?;
        }
        Ok(())
    }

    /// 指定された空間IDセット（`ids`）のうち、まだ値が存在しないIDに対してのみ `data` を書き込む。
    /// 既存の値は上書きされずにそのまま保持される。
    #[tracing::instrument(skip_all)]
    pub fn data_upsert(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let by_leaf = self.group_by_leaf(table_id, ids.flex_ids())?;
        let data_vec = data.to_vec();
        for (region, flex_ids) in by_leaf {
            let map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            self.apply_leaf(table_id, data_type, region, map, &flex_ids, |m| {
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

    /// 指定された空間IDセット（`ids`）に紐づく値をすべて削除する。
    /// 削除後にデータ量が少なくなった場合、リーフ（シャード）の結合（マージ）を試みる。
    #[tracing::instrument(skip_all)]
    pub fn data_remove(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        let by_leaf = self.group_by_leaf(table_id, ids.flex_ids())?;
        let mut affected: Vec<FlexId> = Vec::new();
        for (region, flex_ids) in by_leaf {
            let map =
                shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, &region)?;
            self.apply_leaf(table_id, data_type, region, map, &flex_ids, |m| {
                for flex_id in &flex_ids {
                    m.remove(flex_id);
                }
            })?;
            affected.push(region);
        }
        for region in affected {
            self.try_merge_up(table_id, data_type, region)?;
        }
        Ok(())
    }

    /// 削除処理の後に呼ばれ、データ量が減ったリーフ（シャード）を親ノードへと統合（マージ）する。
    ///
    /// 指定された `region` から親ノードへ遡り、その親に属するすべての子リーフのデータ件数の合計が
    /// 閾値（`MERGE_FLEX_ID_THRESHOLD`）以下であれば、それらを1つのリーフにまとめる。
    /// 統合が起きた場合はさらにその上の親へと再帰的にマージを試み、可能な限りツリーを圧縮する。
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
            let mut combined = 0usize;
            let mut mergeable = true;
            for cr in &child_regions {
                // 空領域のKeyはそもそも存在しないのでスキップ
                let Some(bytes) = self.db.tables_data.get(&self.write_txn, &(table_id, *cr))?
                else {
                    continue;
                };
                match ShardEntry::leaf_count(bytes)? {
                    Some(count) => {
                        combined += count as usize;
                        if combined > MERGE_FLEX_ID_THRESHOLD {
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

            // マージ可能確定後、子マップを実際にデシリアライズする
            let mut child_maps: Vec<SpatialIdMap<Vec<u8>>> = Vec::new();
            for cr in &child_regions {
                let map =
                    shard::load_leaf_map(&self.db.tables_data, &self.write_txn, table_id, cr)?;
                if !map.is_empty() {
                    child_maps.push(map);
                }
            }

            // 値インデックス用に、変更前の全子リーフのキーを集める。
            let mut old_keys = FxHashSet::default();
            for m in &child_maps {
                for (f, v) in m.iter() {
                    old_keys.insert(value_index::make_key(
                        table_id,
                        &value_index::order_preserving(data_type, v),
                        &f,
                    ));
                }
            }

            // 子リーフ群を親領域へ畳み込む
            let merged = SpatialIdMap::merge_shards(parent_region, child_maps)?;

            let mut new_keys = FxHashSet::default();
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

            // 親が新たなリーフになった → さらに上へ伝播。
            region = parent_region;
        }
        Ok(())
    }

    /// 1つのリーフに対するデータ変更（追加・削除など）を適用し、インデックスとストレージに保存する共通処理。
    ///
    /// 変更前の値と変更後の値を比較して、値インデックス（`value_index`）の差分更新を効率的に行ったうえで、
    /// シャードの分割判定や保存処理（`store_shard`）へ引き継ぐ。
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
        let scan: SpatialIdSet = input.iter().cloned().collect();

        // 変更前の重なりリーフからインデックスキーを計算
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

        // 変更後の重なりリーフからインデックスキーを計算
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

        // 保存（過大なら2分割、空なら削除）。
        self.store_shard(table_id, region, map)?;

        Ok(())
    }

    /// 値インデックス（`value_index`）の差分更新を行う。
    ///
    /// 変更前（`old_keys`）と変更後（`new_keys`）のキーセットを比較し、不要になったキーの削除と、
    /// 新たに増えたキーの追加を行う。
    /// LMDB（B-tree）の特性上、ランダムな順序でアクセスするよりもソート済みの順序でアクセスした方が
    /// キャッシュ効率が高いため、削除・追加ともにキーを昇順にソートしてから適用する。
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

    /// 入力された複数の空間ID（`FlexId`）を、現在データベース内に存在するリーフ領域（シャード）ごとにグループ化する。
    /// ポインタツリーのルートから一度の降下で効率的に分類（ルーティング）を行う。
    fn group_by_leaf(
        &self,
        table_id: TableId,
        flex_ids: impl Iterator<Item = FlexId>,
    ) -> Result<FxHashMap<FlexId, Vec<FlexId>>, AppError> {
        let ids: Vec<FlexId> = flex_ids.collect();
        shard::route_leaves_batched(&self.db.tables_data, &self.write_txn, table_id, ids.iter())
    }

    /// 変更されたリーフ（シャード）をデータベースに保存する。
    /// データ件数が閾値（`MAX_FLEX_ID_PER_SHARD`）を超えている場合は、シャードを分割（Split）する。
    ///
    /// **分割時の工夫（パス圧縮）:**
    /// 空間ツリーは理論上 F(階層) → X → Y の順で交互に分割されるが、データが偏っていて
    /// 「片側が空になる分割（退化分割）」が発生する場合は、無駄な中間ポインタノードを作らずに
    /// 子ノードを上位へ直接繋ぐ（巻き上げ）。
    /// これにより、実際にデータが分かれる軸でのみポインタノードが作られ、ツリーの階層が浅く保たれる。
    /// 空になったシャード（データ件数0）はデータベースから削除される。
    ///
    /// 前提として、本関数が呼ばれる時点では `region` は必ずリーフノード（または新規作成ノード）である。
    fn store_shard(
        &mut self,
        table_id: TableId,
        region: FlexId,
        map: SpatialIdMap<Vec<u8>>,
    ) -> Result<(), AppError> {
        let key = (table_id, region);

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

    /// データが閾値を超えた `map` を2つに分割し、分割されたそれぞれの子リーフを再帰的に評価・処理する。
    /// 処理された子リーフの領域IDは、親のポインタノードが持つべき「子領域のリスト（`out`）」に追加される。
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

    /// 分割された子シャードを評価し、保存するかさらに分割するかを決定する処理（パス圧縮の本体）。
    /// 以下の条件で再帰的に処理を行う。
    /// 1. **空の場合**: データベースから削除し、領域IDのみを親のリスト（`out`）に積む。
    /// 2. **閾値内の場合**: そのままリーフとして保存し、領域IDを親のリスト（`out`）に積む。
    /// 3. **閾値超過で、片側が空になる場合（退化分割）**: 中間ノードを作らずにさらに分割し、孫ノードを直接現在の親リスト（`out`）に積む。
    /// 4. **閾値超過で、両側にデータがある場合（実分割）**: 新たなポインタノードとして保存し、その領域IDを親リスト（`out`）に積む。
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
                &(table_id, cr),
                &ShardEntry::encode_pointers(&grand),
            )?;
            out.push(cr);
        }
        Ok(())
    }

    /// リーフノードのデータをシリアライズし、先頭にデータ件数をヘッダとして付与してデータベースに保存する。
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
            &(table_id, *region),
            &ShardEntry::encode_leaf(map.count() as u32, &bytes),
        )?;
        Ok(())
    }
}
