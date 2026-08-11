//! FlexTree（シャードツリー）のデータ操作。
//!
//! ノードのバイト表現は [`shard_entry`](crate::repositories::encoding::shard_entry) に
//! 共通化してあり、TiKV 実装と同じ形式でシャードを読み書きする。違うのはノードの
//! 取得手段（mmap 上のゼロコピー参照か、ネットワーク越しの取得か）だけ。

use rustc_hash::{FxHashMap, FxHashSet};

use kasane_logic::{FlexId, SpatialIdMap, SpatialIdSet};

use crate::error::AppError;
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::repositories::ValueGroups;
use crate::repositories::encoding::shard_entry::{
    MAX_FLEX_ID_PER_SHARD, MERGE_FLEX_ID_THRESHOLD, ShardEntry,
};
use crate::repositories::encoding::value_index;

use super::shard;
use super::{KasaneDbRead, KasaneDbWrite};

type ValueMap = FxHashMap<Vec<u8>, Vec<FlexId>>;

/// `data_get` を葉単位で Rayon 並列化する基準（触れる**リーフ数**）。
///
/// 並列化の基準は「クエリ FlexId 数」ではなく「実際に触れる葉の数」に置く。
/// 広域検索はクエリ FlexId が少数（数個）でも数千の葉・数百万 FlexId に及ぶため、
/// クエリ数で判定すると単一スレッドに落ちてしまう（実測: 540万 FlexId で 363ms）。
const DATA_GET_LEAF_PARALLEL_THRESHOLD: usize = 32;

impl<'a> KasaneDbRead<'a> {
    /// 指定された範囲の空間IDを値ごとにグループ化して返す。
    ///
    /// ルーティング（ポインタ木の降下）は 1 回だけ行い、担当リーフへ振り分けたあと、
    /// **リーフ単位**で解決を並列化する。各リーフは互いに独立（同一 FlexId は 1 つの葉にしか
    /// 属さない）なので、部分マップを作って最後に値でマージするだけで正しく合流できる。
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

        // 悪意のあるユーザーによる数百万件の `flex_ids` の全件ルーティング（DoS攻撃）を防ぐため、
        // チャンク単位で遅延評価し、limit に達したら残りのルーティングを打ち切る。
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

            // ルーティングはバッチ単位で行い、大量件数時の CPU 浪費を防ぐ。
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

                // 葉を（スレッド数で割った）チャンクに分け、チャンクごとに 1 つの読み取り txn を
                // 開いて担当リーフを解決する。txn 開設コストを葉数分ではなくスレッド数分に抑える。
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

    /// 1 つのリーフ領域を読み、そこへ振り分けられた各クエリ FlexId を解決して
    /// `by_value`（値→FlexId 群）へ蓄積する。
    ///
    /// FlexId ごとに値バイト列でハッシュすると、値の種類が少数でも巨大な結果（数百万 FlexId ）で
    /// 同じバイト列を何百万回もハッシュすることになる。そこでまず**この葉ローカルの辞書
    /// インデックス（u32）**でグルーピングし（整数ハッシュは軽い）、葉に現れた distinct 値の
    /// 数だけ実バイト列へ復元して全体マップへマージする。
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

        // 葉ローカルの辞書 index で集約（中間 Vec を作らず整数キーでハッシュ）。
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

        // 葉の distinct 値だけ実バイト列へ復元して全体マップへマージ。
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
    /// 値が `value` と等しい FlexId の [`FlexId`] を集める。
    ///
    /// ストレージのカーソルを呼び出し側へ渡さないよう、結果は確定した `Vec` で返す
    /// （バックエンドによっては結果がネットワーク越しに届くため、遅延イテレータを
    /// トランザクション境界の外へ持ち出せない）。
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

    /// 値が `lo`〜`hi`（両端含む）に入る FlexId の [`FlexId`] を集める。
    ///
    /// インデックスキーは `table_id ‖ vkey ‖ flexid` と値を可変長のまま連結しているため、
    /// 可変長型では**バイト範囲だけでは絞りきれない**。`vkey` が `hi` の真の接頭辞に
    /// なっている行は、続く flexid のバイト次第で `hi ‖ 0xFF…` を超えた位置に並びうる
    /// （例: `hi = "bz"` に対する値 `"b"` は、flexid が `0x7A` より大きいと範囲外に出る）。
    /// そこで型の幅で読む範囲を決め、取り出した `vkey` で最終的に絞る。
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
            // 可変長では該当キーがバイト順で連続しないため、過不足なしにはできない。
            // 「必ず覆う最小の範囲」まで絞り、あとは `keep` の厳密判定に任せる。
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
    /// 指定された空間IDセット（`ids`）すべてに対して `data` を書き込む。
    /// 既に値が存在するIDについては、新しい値で上書きする。
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

    /// 指定された空間IDセット（`ids`）のうち、まだ値が存在しないIDに対してのみ `data` を書き込む。
    /// 既存の値は上書きされずにそのまま保持される。
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

    /// 指定された空間IDセット（`ids`）に紐づく値をすべて削除する。
    /// 削除後にデータ量が少なくなった場合、リーフ（シャード）の結合（マージ）を試みる。
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

    /// 削除処理の後に呼ばれ、データ量が減ったリーフ（シャード）を親ノードへと統合（マージ）する。
    ///
    /// 指定された `region` から親ノードへ遡り、その親に属するすべての子リーフのデータ件数の合計が
    /// 閾値（`MERGE_FLEX_ID_THRESHOLD`）以下であれば、それらを1つのリーフにまとめる。
    /// 統合が起きた場合はさらにその上の親へと再帰的にマージを試み、可能な限りツリーを圧縮する。
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
            // 索引を維持しないテーブルでは、統合は木の形を変えるだけで
            // `(FlexId, 値)` の対応は変わらないので、差分計算そのものが要らない。
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

            // 子リーフ群を親領域へ畳み込む
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
                // 値インデックス差分更新
                self.update_value_index(old_keys, new_keys)?;
            }

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
        index: Option<TableDataType>,
        region: FlexId,
        mut map: SpatialIdMap<Vec<u8>>,
        input: &[FlexId],
        modify: F,
    ) -> Result<(), AppError>
    where
        F: FnOnce(&mut SpatialIdMap<Vec<u8>>),
    {
        // 索引を維持しないテーブルでは、差分計算そのものを飛ばす。索引キーは格納
        // `FlexId` 1 件につき 1 つ増えるので、ここを通るかどうかで 1 回の書き込みが
        // 触るキー数が 3 桁変わる。
        let Some(data_type) = index else {
            modify(&mut map);
            return self.store_shard(table_id, region, map);
        };

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
