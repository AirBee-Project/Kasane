//! FlexTree（シャードツリー）のデータ操作。
//!
//! ツリーの形と分割・統合の規則は LMDB 実装と同一で、違うのは「ノードをどう読み書きするか」
//! だけ。ノードのバイト表現は [`shard_entry`](crate::repositories::encoding::shard_entry) に
//! 共通化してあるので、両バックエンドで同じデータ形式になる。
//!
//! # LMDB 実装との違い
//!
//! - ノードの取得がネットワーク越しになるため、木の降下では**同じ深さのノードをまとめて**
//!   取得する（`batch_get`）。1 ノードずつ引くと深さ × 往復のレイテンシがかかる。
//! - ゼロコピー（mmap 上の `ArchivedSpatialIdMap` を直接読む）は使えないので、
//!   常に所有バイト列から `SpatialIdMap` を復元する。
//! - 再帰は `Box::pin` で明示的に間接化する（async fn の再帰のため）。

use kasane_logic::{FlexId, RangeId, SpatialIdMap, SpatialIdSet};
use rustc_hash::{FxHashMap, FxHashSet};
use tokio::sync::Mutex;

use crate::error::AppError;
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::repositories::ValueGroups;
use crate::repositories::encoding::flat_keys::{self, LockScope};
use crate::repositories::encoding::shard_entry::{
    MAX_FLEX_ID_PER_SHARD, MERGE_FLEX_ID_THRESHOLD, ShardEntry,
};
use crate::repositories::encoding::value_index;

use super::{TikvRead, TikvWrite, kv};

type Txn = Mutex<tikv_client::Transaction>;

// --- ノードの読み書き ---

async fn load_node(
    txn: &Txn,
    table_id: TableId,
    region: &FlexId,
) -> Result<Option<Vec<u8>>, AppError> {
    kv::get(txn, flat_keys::shard(table_id, region)).await
}

/// 複数領域のノードをまとめて取得する。存在しない領域は結果に含まれない。
///
/// 呼び出し側はキーではなく領域で引きたいので、領域をキーにして返す
/// （キーで返すと、引くたびにキーを組み立て直すことになる）。
async fn load_nodes(
    txn: &Txn,
    table_id: TableId,
    regions: &[FlexId],
) -> Result<FxHashMap<FlexId, Vec<u8>>, AppError> {
    let by_key: FxHashMap<Vec<u8>, FlexId> = regions
        .iter()
        .map(|r| (flat_keys::shard(table_id, r), *r))
        .collect();
    let pairs = kv::batch_get(txn, by_key.keys().cloned().collect()).await?;
    Ok(pairs
        .into_iter()
        .filter_map(|(key, value)| by_key.get(&key).map(|region| (*region, value)))
        .collect())
}

/// リーフのバイト列から [`SpatialIdMap`] を復元する。未作成なら空のマップ。
fn decode_leaf(region: &FlexId, bytes: Option<&[u8]>) -> Result<SpatialIdMap<Vec<u8>>, AppError> {
    let Some(bytes) = bytes else {
        return Ok(SpatialIdMap::new_in_shard(*region));
    };
    match ShardEntry::leaf_payload(bytes)? {
        // 自分自身が書いたバイト列。形式バージョンは検証されるので、
        // 古い形式が黙って誤読されることはない。
        Some(map_bytes) => unsafe { SpatialIdMap::<Vec<u8>>::from_bytes(map_bytes) }
            .map_err(|e| AppError::InternalError(format!("rkyv deserialize: {e}"))),
        None => Err(AppError::InternalError(
            "routed to a pointer node".to_string(),
        )),
    }
}

// --- ルーティング ---

/// 振り分け先のリーフ。降下の途中で読んだノードのバイト列を持ち回るので、
/// 呼び出し側が同じキーを引き直さずに済む。
pub(super) struct RoutedLeaf {
    pub region: FlexId,
    /// 到達した `flex_id` 群。
    pub queries: Vec<FlexId>,
    /// リーフのバイト列。未作成領域なら `None`。
    pub node: Option<Vec<u8>>,
}

impl RoutedLeaf {
    fn leaf_map(&self) -> Result<SpatialIdMap<Vec<u8>>, AppError> {
        decode_leaf(&self.region, self.node.as_deref())
    }
}

/// 複数の `flex_id` を木の降下でまとめて振り分ける。
///
/// 書き込み経路でも使うため、まだノードが作られていない領域も担当リーフとして返す。
async fn route_leaves_batched(
    txn: &Txn,
    table_id: TableId,
    ids: &[FlexId],
) -> Result<Vec<RoutedLeaf>, AppError> {
    // f 符号で上下半球に分け、各半球ルートから 1 回ずつ降りる。
    let mut lower = Vec::new();
    let mut upper = Vec::new();
    for f in ids {
        if f.f_index().is_negative() {
            lower.push(*f);
        } else {
            upper.push(*f);
        }
    }

    let mut out: FxHashMap<FlexId, RoutedLeaf> = FxHashMap::default();
    descend_batched(txn, table_id, FlexId::LOWER_MAX, lower, &mut out).await?;
    descend_batched(txn, table_id, FlexId::UPPER_MAX, upper, &mut out).await?;
    Ok(out.into_values().collect())
}

/// `region` を根として `ids` を子へ振り分けながら降りる。
///
/// 幅優先で「同じ深さのノードをまとめて取得」してから振り分けることで、
/// ネットワーク往復を木の深さ分に抑える。
async fn descend_batched(
    txn: &Txn,
    table_id: TableId,
    root: FlexId,
    ids: Vec<FlexId>,
    out: &mut FxHashMap<FlexId, RoutedLeaf>,
) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }

    // (領域, そこへ到達した flex_id 群) を深さごとに処理する。
    let mut level: Vec<(FlexId, Vec<FlexId>)> = vec![(root, ids)];

    while !level.is_empty() {
        let regions: Vec<FlexId> = level.iter().map(|(r, _)| *r).collect();
        let mut nodes = load_nodes(txn, table_id, &regions).await?;

        let mut next: Vec<(FlexId, Vec<FlexId>)> = Vec::new();
        for (region, bucket) in level {
            let node = nodes.remove(&region);
            let children = match &node {
                // 未作成領域 or 実データリーフ → ここへ到達した全 flex_id が担当。
                None => None,
                Some(bytes) => ShardEntry::child_pointers(bytes)?,
            };

            match children {
                None => {
                    // リーフのバイト列はここで確定するので持たせておく（再取得しない）。
                    out.entry(region)
                        .or_insert_with(|| RoutedLeaf {
                            region,
                            queries: Vec::new(),
                            node,
                        })
                        .queries
                        .extend(bucket);
                }
                Some(children) => {
                    for child in children {
                        let sub: Vec<FlexId> = bucket
                            .iter()
                            .filter(|f| child.intersection(f).is_some())
                            .copied()
                            .collect();
                        if !sub.is_empty() {
                            next.push((child, sub));
                        }
                    }
                }
            }
        }
        level = next;
    }

    Ok(())
}

/// `range` と重なる**既存のリーフ領域**を集める（読み取り経路）。
async fn route_leaves_for_range(
    txn: &Txn,
    table_id: TableId,
    range: &RangeId,
) -> Result<Vec<(FlexId, Vec<u8>)>, AppError> {
    let mut out = Vec::new();
    let mut level: Vec<FlexId> = [FlexId::LOWER_MAX, FlexId::UPPER_MAX]
        .into_iter()
        .filter(|root| root.intersects_range(range))
        .collect();

    while !level.is_empty() {
        let mut nodes = load_nodes(txn, table_id, &level).await?;

        let mut next: Vec<FlexId> = Vec::new();
        for region in level {
            // 未作成領域＝データ無し。読み取りでは辿る必要がない。
            let Some(bytes) = nodes.remove(&region) else {
                continue;
            };
            match ShardEntry::child_pointers(&bytes)? {
                // 読んだバイト列をそのまま返し、呼び出し側の再取得をなくす。
                None => out.push((region, bytes)),
                Some(children) => {
                    next.extend(children.into_iter().filter(|c| c.intersects_range(range)));
                }
            }
        }
        level = next;
    }

    Ok(out)
}

/// `region` を直接の子に持つ親ポインタノードを見つけ、`(親領域, 親の全子領域)` を返す。
async fn find_parent_pointer(
    txn: &Txn,
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
        let Some(bytes) = load_node(txn, table_id, &cur).await? else {
            return Ok(None);
        };
        let Some(children) = ShardEntry::child_pointers(&bytes)? else {
            return Ok(None);
        };
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
}

// --- 読み取り ---

impl TikvRead<'_> {
    pub(super) async fn data_get_impl(
        &self,
        table_id: TableId,
        ids: SpatialIdSet,
        limit: Option<usize>,
    ) -> Result<ValueGroups, AppError> {
        let mut by_value: FxHashMap<Vec<u8>, Vec<FlexId>> = FxHashMap::default();
        let mut held = 0usize;

        // 大量の flex_id を一度に全件ルーティングしないよう、チャンクに区切って
        // limit へ達した時点で打ち切る。
        const ROUTING_BATCH_SIZE: usize = 65536;
        let mut iter = ids.flex_ids();

        'outer: loop {
            if limit.is_some_and(|l| held >= l) {
                break;
            }
            let batch: Vec<FlexId> = iter.by_ref().take(ROUTING_BATCH_SIZE).collect();
            if batch.is_empty() {
                break;
            }

            for leaf in route_leaves_batched(&self.txn, table_id, &batch).await? {
                let map = leaf.leaf_map()?;
                if map.is_empty() {
                    continue;
                }
                for query in &leaf.queries {
                    for (got, value) in map.get(query) {
                        // 値の複製は distinct な値の分だけで済ませる（セル数ではなく）。
                        match by_value.get_mut(value) {
                            Some(ids) => ids.push(got),
                            None => {
                                by_value.insert(value.clone(), vec![got]);
                            }
                        }
                        held += 1;
                        if limit.is_some_and(|l| held >= l) {
                            break 'outer;
                        }
                    }
                }
            }
        }

        Ok(by_value.into_iter().collect())
    }

    pub(super) async fn data_filter_eq_impl(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        value: &[u8],
    ) -> Result<Vec<FlexId>, AppError> {
        let vkey = value_index::order_preserving(data_type, value);
        let prefix = flat_keys::value_index_prefix(table_id, &vkey);
        let keys = kv::scan_prefix_keys(&self.txn, &prefix).await?;

        let mut out = Vec::new();
        for key in keys {
            // 可変長値で前方一致しただけの別キーを除外（残りがちょうど flexid 分の長さ）。
            if key.len() != prefix.len() + FlexId::ENCODED_LEN {
                continue;
            }
            out.push(value_index::flexid_from_key(&key[1..])?);
        }
        Ok(out)
    }

    pub(super) async fn data_filter_range_impl(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        lo: &[u8],
        hi: &[u8],
    ) -> Result<Vec<FlexId>, AppError> {
        let start =
            flat_keys::value_index_prefix(table_id, &value_index::order_preserving(data_type, lo));
        // hi 側は flexid 部を最大化して `(hi, *)` まで含める。
        let mut end =
            flat_keys::value_index_prefix(table_id, &value_index::order_preserving(data_type, hi));
        end.extend_from_slice(&[0xFF; FlexId::ENCODED_LEN]);

        let keys = kv::scan_inclusive_keys(&self.txn, start, end).await?;
        keys.iter()
            .map(|key| value_index::flexid_from_key(&key[1..]))
            .collect()
    }

    /// クエリ実行器の入力として、指定範囲のセルを読み出す。
    pub(super) async fn read_cells_in_range(
        &self,
        table_id: TableId,
        range: &RangeId,
    ) -> Result<Vec<(FlexId, Vec<u8>)>, AppError> {
        let leaves = route_leaves_for_range(&self.txn, table_id, range).await?;
        let mut out = Vec::new();
        for (region, bytes) in leaves {
            let map = decode_leaf(&region, Some(&bytes))?;
            for (id, value) in map.get(range) {
                out.push((id, value.clone()));
            }
        }
        Ok(out)
    }
}

// --- 書き込み ---

impl TikvWrite<'_> {
    pub(super) async fn data_insert_impl(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        self.require_lock(LockScope::Table, &table_id.into_bytes())?;

        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();
        for leaf in route_leaves_batched(&self.txn, table_id, &flex_ids).await? {
            let map = leaf.leaf_map()?;
            let targets = leaf.queries;
            self.apply_leaf(table_id, data_type, leaf.region, map, &targets, |m| {
                for flex_id in &targets {
                    m.insert(*flex_id, data.to_vec());
                }
            })
            .await?;
        }
        Ok(())
    }

    pub(super) async fn data_upsert_impl(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        self.require_lock(LockScope::Table, &table_id.into_bytes())?;

        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();
        let data_vec = data.to_vec();
        for leaf in route_leaves_batched(&self.txn, table_id, &flex_ids).await? {
            let map = leaf.leaf_map()?;
            let targets = leaf.queries;
            self.apply_leaf(table_id, data_type, leaf.region, map, &targets, |m| {
                let mut target_set = SpatialIdSet::new();
                for flex_id in &targets {
                    let occupied: SpatialIdSet = m.get(flex_id).map(|(f, _)| f).collect();
                    target_set.clear();
                    target_set.insert(*flex_id);
                    for f in (&target_set - &occupied).flex_ids() {
                        m.insert(f, data_vec.clone());
                    }
                }
            })
            .await?;
        }
        Ok(())
    }

    pub(super) async fn data_remove_impl(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        self.require_lock(LockScope::Table, &table_id.into_bytes())?;

        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();
        let mut affected: Vec<FlexId> = Vec::new();
        for leaf in route_leaves_batched(&self.txn, table_id, &flex_ids).await? {
            let map = leaf.leaf_map()?;
            let targets = leaf.queries;
            let region = leaf.region;
            self.apply_leaf(table_id, data_type, region, map, &targets, |m| {
                for flex_id in &targets {
                    m.remove(flex_id);
                }
            })
            .await?;
            affected.push(region);
        }
        for region in affected {
            self.try_merge_up(table_id, data_type, region).await?;
        }
        Ok(())
    }

    /// 1 つのリーフへの変更を適用し、値インデックスを差分更新してから保存する。
    async fn apply_leaf<F>(
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

        // 変更前の重なりリーフからインデックスキーを計算。
        let mut old_keys = FxHashSet::default();
        let mut pre_modify_scan = scan.clone();
        for f_scan in scan.iter() {
            for (f, v) in map.get_overlapping(&f_scan) {
                old_keys.insert(index_key(table_id, data_type, v, &f));
                pre_modify_scan.insert(f);
            }
        }

        modify(&mut map);

        // 変更後の重なりリーフからインデックスキーを計算。
        let mut new_keys = FxHashSet::default();
        for f_scan in pre_modify_scan.iter() {
            for (f, v) in map.get_overlapping(&f_scan) {
                new_keys.insert(index_key(table_id, data_type, v, &f));
            }
        }

        self.update_value_index(old_keys, new_keys).await?;
        self.store_shard(table_id, region, map).await
    }

    /// 値インデックスの差分だけを反映する。
    async fn update_value_index(
        &mut self,
        old_keys: FxHashSet<Vec<u8>>,
        new_keys: FxHashSet<Vec<u8>>,
    ) -> Result<(), AppError> {
        // 昇順に適用する（キーが連続していた方がリージョン跨ぎのアクセスが減る）。
        let mut to_delete: Vec<&Vec<u8>> = old_keys.difference(&new_keys).collect();
        to_delete.sort_unstable();
        for key in to_delete {
            kv::delete(&self.txn, key.clone()).await?;
        }

        let mut to_put: Vec<&Vec<u8>> = new_keys.difference(&old_keys).collect();
        to_put.sort_unstable();
        for key in to_put {
            kv::put(&self.txn, key.clone(), Vec::new()).await?;
        }
        Ok(())
    }

    /// 変更後のリーフを保存する。過大なら分割し、空なら削除する。
    async fn store_shard(
        &mut self,
        table_id: TableId,
        region: FlexId,
        map: SpatialIdMap<Vec<u8>>,
    ) -> Result<(), AppError> {
        if !map.should_split_shard(MAX_FLEX_ID_PER_SHARD) {
            if map.is_empty() {
                kv::delete(&self.txn, flat_keys::shard(table_id, &region)).await?;
            } else {
                self.put_leaf(table_id, &region, &map).await?;
            }
            return Ok(());
        }

        // 分割が必要 → パス圧縮した被覆子領域を構築し、親をポインタノードにする。
        let mut children = Vec::new();
        let ((lo_r, lo), (hi_r, hi)) = map
            .split_shard()
            .ok_or_else(|| AppError::InternalError("split on shardless map".to_string()))?;
        self.emit_child(table_id, lo_r, lo, &mut children).await?;
        self.emit_child(table_id, hi_r, hi, &mut children).await?;

        kv::put(
            &self.txn,
            flat_keys::shard(table_id, &region),
            ShardEntry::encode_pointers(&children),
        )
        .await
    }

    /// 分割された子シャードを保存するか、さらに分割するかを決める（パス圧縮の本体）。
    fn emit_child<'s>(
        &'s mut self,
        table_id: TableId,
        cr: FlexId,
        cm: SpatialIdMap<Vec<u8>>,
        out: &'s mut Vec<FlexId>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 's>> {
        // async fn の再帰なので明示的に間接化する。
        Box::pin(async move {
            if cm.is_empty() {
                // 空領域：被覆として領域だけ積む。万一の古いキーは消す。
                kv::delete(&self.txn, flat_keys::shard(table_id, &cr)).await?;
                out.push(cr);
                return Ok(());
            }
            if !cm.should_split_shard(MAX_FLEX_ID_PER_SHARD) {
                self.put_leaf(table_id, &cr, &cm).await?;
                out.push(cr);
                return Ok(());
            }

            // 過大：1 段だけ覗いて、退化分割か実分割かを決める。
            let ((clo_r, clo), (chi_r, chi)) = cm
                .split_shard()
                .ok_or_else(|| AppError::InternalError("split on shardless map".to_string()))?;

            if clo.is_empty() || chi.is_empty() {
                // 退化分割：中間ポインタを作らず孫を巻き上げる（チェーン圧縮）。
                self.emit_child(table_id, clo_r, clo, out).await?;
                self.emit_child(table_id, chi_r, chi, out).await?;
            } else {
                // 実分割：cr を独立ポインタノードにする。
                let mut grand = Vec::new();
                self.emit_child(table_id, clo_r, clo, &mut grand).await?;
                self.emit_child(table_id, chi_r, chi, &mut grand).await?;
                kv::put(
                    &self.txn,
                    flat_keys::shard(table_id, &cr),
                    ShardEntry::encode_pointers(&grand),
                )
                .await?;
                out.push(cr);
            }
            Ok(())
        })
    }

    /// リーフを件数ヘッダ付きで保存する。
    async fn put_leaf(
        &mut self,
        table_id: TableId,
        region: &FlexId,
        map: &SpatialIdMap<Vec<u8>>,
    ) -> Result<(), AppError> {
        let bytes = map
            .to_bytes()
            .map_err(|e| AppError::InternalError(format!("rkyv serialize: {e}")))?;
        kv::put(
            &self.txn,
            flat_keys::shard(table_id, region),
            ShardEntry::encode_leaf(map.count() as u32, &bytes),
        )
        .await
    }

    /// 削除でデータ量が減ったリーフを親へ統合し、可能な限り木を圧縮する。
    async fn try_merge_up(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        region: FlexId,
    ) -> Result<(), AppError> {
        let mut region = region;
        loop {
            let Some((parent_region, child_regions)) =
                find_parent_pointer(&self.txn, table_id, &region).await?
            else {
                break;
            };

            // 子を走査し、いずれかがポインタノードならこのレベルは統合しない。
            // 全リーフで合算が閾値以下なら 1 リーフへ畳み込む。
            let nodes = load_nodes(&self.txn, table_id, &child_regions).await?;
            let mut combined = 0usize;
            let mut mergeable = true;
            for cr in &child_regions {
                // 空領域のキーはそもそも存在しないのでスキップ。
                let Some(bytes) = nodes.get(cr) else {
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

            // マージ可能が確定してから、子マップを復元する。
            // バイト列は上の一括取得で手元にあるので引き直さない。
            let mut child_maps: Vec<SpatialIdMap<Vec<u8>>> = Vec::new();
            for cr in &child_regions {
                let map = decode_leaf(cr, nodes.get(cr).map(Vec::as_slice))?;
                if !map.is_empty() {
                    child_maps.push(map);
                }
            }

            let mut old_keys = FxHashSet::default();
            for m in &child_maps {
                for (f, v) in m.iter() {
                    old_keys.insert(index_key(table_id, data_type, v, &f));
                }
            }

            let merged = SpatialIdMap::merge_shards(parent_region, child_maps)?;

            let mut new_keys = FxHashSet::default();
            for (f, v) in merged.iter() {
                new_keys.insert(index_key(table_id, data_type, v, &f));
            }

            self.update_value_index(old_keys, new_keys).await?;

            // 親キーをリーフ（空なら削除）に置換し、子キーを削除する。
            if merged.is_empty() {
                kv::delete(&self.txn, flat_keys::shard(table_id, &parent_region)).await?;
            } else {
                self.put_leaf(table_id, &parent_region, &merged).await?;
            }
            for cr in &child_regions {
                kv::delete(&self.txn, flat_keys::shard(table_id, cr)).await?;
            }

            // 親が新たなリーフになった → さらに上へ伝播。
            region = parent_region;
        }
        Ok(())
    }

    /// 制約変更時に、既存の格納値が新しい制約を満たすか検証する。
    pub(super) async fn validate_existing_data(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        constraints: Option<&crate::models::database::table::TableConstraints>,
    ) -> Result<(), AppError> {
        let entries = kv::scan_prefix(&self.txn, &flat_keys::shards_of(table_id)).await?;
        for (_, bytes) in entries {
            let ShardEntry::Leaf(map_bytes) = ShardEntry::decode(&bytes)? else {
                continue;
            };
            let map = unsafe { SpatialIdMap::<Vec<u8>>::from_bytes(&map_bytes) }
                .map_err(|e| AppError::InternalError(format!("rkyv deserialize: {e}")))?;
            for (_, stored) in map.iter() {
                let restored = crate::services::helpers::value::restore_value(
                    data_type,
                    constraints,
                    stored.as_slice(),
                )?;
                crate::services::helpers::value::interpret_value(data_type, constraints, restored)?;
            }
        }
        Ok(())
    }
}

/// 値インデックスのキーを組み立てる。
fn index_key(
    table_id: TableId,
    data_type: TableDataType,
    value: &[u8],
    flex_id: &FlexId,
) -> Vec<u8> {
    flat_keys::value_index(
        table_id,
        &value_index::order_preserving(data_type, value),
        flex_id,
    )
}
