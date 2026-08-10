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
//! - mmap 上の `ArchivedSpatialIdMap` を直接読むことはできないので、常に受信バッファから
//!   `SpatialIdMap` を復元する。ただし**受信バッファへのゼロコピー**は保たれる
//!   （[`kv::ShardValue`] が検証済みペイロードへの借用を返す）。
//! - 受信バッファは信用できないので、rkyv の非検証アクセスへ渡す前に
//!   [`kv::ShardValue`] の完全性検証を通す（`kv.rs` のフレームの節を参照）。
//! - 再帰は `Box::pin` で明示的に間接化する（async fn の再帰のため）。

use kasane_logic::{FlexId, RangeId, SpatialIdMap, SpatialIdSet};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeSet;
use tokio::sync::Mutex;

use super::keys;
use crate::error::AppError;
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::repositories::ValueGroups;
use crate::repositories::encoding::shard_entry::{
    MAX_FLEX_ID_PER_SHARD, MERGE_FLEX_ID_THRESHOLD, ShardEntry,
};
use crate::repositories::encoding::value_index;

use super::kv::{Reader, ShardValue};
use super::{TikvRead, TikvWrite, kv};

// --- ノードの読み書き ---

async fn load_node<R: Reader>(
    txn: &Mutex<R>,
    table_id: TableId,
    region: &FlexId,
) -> Result<Option<ShardValue>, AppError> {
    kv::get_shard(txn, keys::shard(table_id, region)).await
}

/// 複数領域のノードをまとめて取得する。存在しない領域は結果に含まれない。
///
/// 呼び出し側はキーではなく領域で引きたいので、領域をキーにして返す
/// （キーで返すと、引くたびにキーを組み立て直すことになる）。
async fn load_nodes<R: Reader>(
    txn: &Mutex<R>,
    table_id: TableId,
    regions: &[FlexId],
) -> Result<FxHashMap<FlexId, ShardValue>, AppError> {
    let by_key: FxHashMap<Vec<u8>, FlexId> = regions
        .iter()
        .map(|r| (keys::shard(table_id, r), *r))
        .collect();
    let pairs = kv::batch_get_shards(txn, by_key.keys().cloned().collect()).await?;
    Ok(pairs
        .into_iter()
        .filter_map(|(key, value)| by_key.get(&key).map(|region| (*region, value)))
        .collect())
}

/// リーフのバイト列から [`SpatialIdMap`] を復元する。未作成なら空のマップ。
fn decode_leaf(region: &FlexId, entry: Option<&[u8]>) -> Result<SpatialIdMap<Vec<u8>>, AppError> {
    let Some(entry) = entry else {
        return Ok(SpatialIdMap::new_in_shard(*region));
    };
    match ShardEntry::leaf_payload(entry)? {
        // SAFETY: `entry` は `kv::ShardValue` の CRC 検証を通ったバイト列で、
        // 保存時に自分が書いたものと一致することが確認済み。形式バージョンは
        // `from_bytes` 側でさらに検証されるので、古い形式が黙って誤読されることもない。
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
    pub node: Option<ShardValue>,
}

impl RoutedLeaf {
    fn leaf_map(&self) -> Result<SpatialIdMap<Vec<u8>>, AppError> {
        decode_leaf(&self.region, self.node.as_ref().map(ShardValue::entry))
    }
}

/// 複数の `flex_id` を木の降下でまとめて振り分ける。
///
/// 書き込み経路でも使うため、まだノードが作られていない領域も担当リーフとして返す。
async fn route_leaves_batched<R: Reader>(
    txn: &Mutex<R>,
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
async fn descend_batched<R: Reader>(
    txn: &Mutex<R>,
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
                Some(value) => ShardEntry::child_pointers(value.entry())?,
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
async fn route_leaves_for_range<R: Reader>(
    txn: &Mutex<R>,
    table_id: TableId,
    range: &RangeId,
) -> Result<Vec<(FlexId, ShardValue)>, AppError> {
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
            let Some(value) = nodes.remove(&region) else {
                continue;
            };
            match ShardEntry::child_pointers(value.entry())? {
                // 読んだバイト列をそのまま返し、呼び出し側の再取得をなくす。
                None => out.push((region, value)),
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
async fn find_parent_pointer<R: Reader>(
    txn: &Mutex<R>,
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
        let Some(value) = load_node(txn, table_id, &cur).await? else {
            return Ok(None);
        };
        let Some(children) = ShardEntry::child_pointers(value.entry())? else {
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

impl<R: Reader> TikvRead<'_, R> {
    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(super) async fn data_get_impl(
        &self,
        table_id: TableId,
        ids: SpatialIdSet,
        limit: Option<usize>,
    ) -> Result<ValueGroups, AppError> {
        let mut by_value: FxHashMap<Vec<u8>, Vec<FlexId>> = FxHashMap::default();
        let mut held = 0usize;

        // 全件をまとめてルーティングせず、チャンクに区切って `limit` へ達した時点で
        // 打ち切る。ここで区切るのは**打ち切りの粒度**と手元に持つ `FlexId` の量で、
        // 1 リクエストのキー数ではない（そちらは `kv::BATCH_KEYS` が別途縛る。
        // 木の同じ深さで引くキー数は相異なる領域の数であって、`flex_id` の数ではない）。
        const ROUTING_BATCH_SIZE: usize = 8192;
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
                        // 値の複製は distinct な値の分だけで済ませる（ FlexId 数ではなく）。
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

    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(super) async fn data_filter_eq_impl(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        value: &[u8],
    ) -> Result<Vec<FlexId>, AppError> {
        let vkey = value_index::order_preserving(data_type, value);
        let prefix = keys::value_index_prefix(table_id, &vkey);
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

    /// 値が `lo`〜`hi`（両端含む）に入る FlexId を引く。
    ///
    /// 値インデックスのキーは `0x07 ‖ table_id ‖ vkey ‖ flexid` と値を可変長のまま
    /// 連結しているため、可変長型では**バイト範囲だけでは絞りきれない**。
    /// `vkey` が `hi` の真の接頭辞になっている行は、続く flexid のバイト次第で
    /// `hi ‖ 0xFF…` を超えた位置に並びうる（例: `hi = "bz"` に対する値 `"b"`）。
    /// そこで型の幅で読む範囲を決め、取り出した `vkey` で最終的に絞る。
    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(super) async fn data_filter_range_impl(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        lo: &[u8],
        hi: &[u8],
    ) -> Result<Vec<FlexId>, AppError> {
        let lo_vkey = value_index::order_preserving(data_type, lo);
        let hi_vkey = value_index::order_preserving(data_type, hi);

        let keys = if data_type.has_fixed_width_value() {
            // 全キーが同じ長さなので、この範囲が過不足なく該当行を覆う。
            let start = keys::value_index_prefix(table_id, &lo_vkey);
            let mut end = keys::value_index_prefix(table_id, &hi_vkey);
            // hi 側は flexid 部を最大化して `(hi, *)` まで含める。
            end.extend_from_slice(&[0xFF; FlexId::ENCODED_LEN]);
            kv::scan_inclusive_keys(&self.txn, start, end).await?
        } else {
            // 可変長では該当キーがバイト順で連続しないため、過不足なしにはできない。
            // 「必ず覆う最小の範囲」まで絞り、あとは下の厳密フィルタに任せる。
            let (start, end) = keys::value_index_scan_bounds(table_id, &lo_vkey, &hi_vkey);
            kv::scan_keys_range(&self.txn, start, end).await?
        };

        let mut out = Vec::new();
        for key in keys {
            let entry = &key[1..];
            let vkey = value_index::vkey_from_key(entry)?;
            if vkey < lo_vkey.as_slice() || vkey > hi_vkey.as_slice() {
                continue;
            }
            out.push(value_index::flexid_from_key(entry)?);
        }
        Ok(out)
    }

    /// クエリ実行器の入力として、指定範囲の FlexId を読み出す。
    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(super) async fn read_flex_ids_in_range(
        &self,
        table_id: TableId,
        range: &RangeId,
    ) -> Result<Vec<(FlexId, Vec<u8>)>, AppError> {
        let leaves = route_leaves_for_range(&self.txn, table_id, range).await?;
        let mut out = Vec::new();
        for (region, value) in leaves {
            let map = decode_leaf(&region, Some(value.entry()))?;
            for (id, value) in map.get(range) {
                out.push((id, value.clone()));
            }
        }
        Ok(out)
    }
}

// --- 書き込み ---
//
// # ロックの取り方
//
// データ書き込みはテーブル全体を排他しない。触るのは「対象の空間 ID が属するリーフ」
// だけで、そのキーを [`kv::lock_shards`]（`batch_get_for_update`）でロックし、
// **ロック時点の最新の内容**を同時に受け取る。別のリーフへの書き込みは、別インスタンス
// からのものであっても並列に流れる。
//
// これが安全なのは、この木が次の不変条件を満たすからである。
//
// > **リーフ R の内容または到達可能性を変える操作は、必ず R 自身を書くか消す。**
//
// - 分割できるのはリーフだけなので、R が分割されるなら R が書き換わる
// - 統合（`try_merge_up`）は親へ畳むときに**子キーを削除**する ＝ R を消す
// - 祖先方向の統合は「子のいずれかがポインタノードなら行わない」ので、R より上を
//   畳むには先に R の親を畳む必要があり、それは R を消すことを意味する
//
// よって R をロックすれば R は誰にも変えられない。木の降下（祖先のポインタノード読み）
// はロックなしのスナップショット読みでよく、古い形で誤ったリーフを選んだ場合は
// ロック後の検証で検出できる。
//
// # 検証に失敗したらどうするか
//
// 同一トランザクション内で降下し直しても、`get` は常に `start_ts` を読むので**同じ
// 古い答えが返る**。そこで [`TikvWrite::mark_stale`] でこの試行を捨て、`Storage::write`
// に新しいスナップショットでやり直させる。新しい `start_ts` は他者のコミットより後に
// なるので、必ず前進する。

impl TikvWrite<'_> {
    /// 対象の `flex_id` 群が属するリーフをロックし、最新の内容とともに返す。
    ///
    /// 降下はロックなしで行い、その結果をロック後に検証する。食い違っていたら
    /// この試行ごと捨てて、新しいスナップショットでやり直す（モジュール上部を参照）。
    async fn lock_target_leaves(
        &mut self,
        table_id: TableId,
        flex_ids: &[FlexId],
    ) -> Result<Vec<RoutedLeaf>, AppError> {
        let mut routed = route_leaves_batched(&self.txn, table_id, flex_ids).await?;
        let regions: BTreeSet<FlexId> = routed.iter().map(|leaf| leaf.region).collect();
        let mut locked = kv::lock_shards(&self.txn, table_id, regions).await?;

        for leaf in &mut routed {
            let entry = locked.remove(&leaf.region).ok_or_else(|| {
                AppError::InternalError("locked shard map is missing a routed region".to_string())
            })?;

            match (&leaf.node, &entry) {
                // ポインタノードになっていた ＝ 降下中に分割された。
                (_, Some(value)) if ShardEntry::child_pointers(value.entry())?.is_some() => {
                    return Err(self.mark_stale().into());
                }
                // 降下時はリーフだったのに消えている ＝ 親へ畳まれた可能性がある。
                // 単に空になって消えただけのこともあるが、区別するよりやり直した方が
                // 確実で、やり直しは 1 周で収束する。
                (Some(_), None) => return Err(self.mark_stale().into()),
                _ => {}
            }

            // 降下時に読んだ本体は捨て、ロック時点の内容へ差し替える。
            // 両方を抱えたままにすると、対象リーフのバイト列を二重に持つことになる。
            leaf.node = entry;
        }
        Ok(routed)
    }

    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(super) async fn data_insert_impl(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();
        for leaf in self.lock_target_leaves(table_id, &flex_ids).await? {
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

    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(super) async fn data_upsert_impl(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();
        let data_vec = data.to_vec();
        for leaf in self.lock_target_leaves(table_id, &flex_ids).await? {
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

    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(super) async fn data_remove_impl(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();
        let mut affected: Vec<FlexId> = Vec::new();
        for leaf in self.lock_target_leaves(table_id, &flex_ids).await? {
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
        // 削除と書き込みをまとめて 1 回のリクエストにする。`write_batch` が
        // キー順に並べ替えるので、キーが連続してリージョン跨ぎのアクセスが減り、
        // 順序が固定されることでデッドロックも避けられる。
        kv::write_batch(
            &self.txn,
            old_keys.difference(&new_keys).cloned(),
            new_keys
                .difference(&old_keys)
                .map(|key| (key.clone(), Vec::new())),
        )
        .await
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
                kv::delete_shard(&self.txn, table_id, &region).await?;
            } else {
                self.put_leaf(table_id, &region, &map).await?;
            }
            return Ok(());
        }

        // 分割が必要 → パス圧縮した被覆子領域を構築し、親をポインタノードにする。
        //
        // 子領域へ他者が到達する経路は今は存在しない（到達するには親がポインタノードで
        // ある必要があり、それを作るのがこの処理自身）。したがって子のロックを別途
        // 取る必要はなく、書き込み時の暗黙ロックで足りる。
        let mut children = Vec::new();
        let ((lo_r, lo), (hi_r, hi)) = map
            .split_shard()
            .ok_or_else(|| AppError::InternalError("split on shardless map".to_string()))?;
        self.emit_child(table_id, lo_r, lo, &mut children).await?;
        self.emit_child(table_id, hi_r, hi, &mut children).await?;

        kv::put_shard(
            &self.txn,
            table_id,
            &region,
            &ShardEntry::encode_pointers(&children),
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
                kv::delete_shard(&self.txn, table_id, &cr).await?;
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
                kv::put_shard(
                    &self.txn,
                    table_id,
                    &cr,
                    &ShardEntry::encode_pointers(&grand),
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
        kv::put_shard(
            &self.txn,
            table_id,
            region,
            &ShardEntry::encode_leaf(map.count() as u32, &bytes),
        )
        .await
    }

    /// 削除でデータ量が減ったリーフを親へ統合し、可能な限り木を圧縮する。
    ///
    /// 統合は親と**その全子**を書き換えるので、判断の前にその集合をまとめてロックする。
    /// 空の子領域もロックの対象に含める（キーが無くてもロックは取れる）。そうしないと、
    /// 統合で消える予定の空領域へ他者が書き込めてしまう。
    async fn try_merge_up(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        region: FlexId,
    ) -> Result<(), AppError> {
        let mut region = region;
        loop {
            let Some((parent_region, descended_children)) =
                find_parent_pointer(&self.txn, table_id, &region).await?
            else {
                break;
            };

            // 親と全子をロックし、ロック時点の内容を得る。
            let mut targets: BTreeSet<FlexId> = descended_children.iter().copied().collect();
            targets.insert(parent_region);
            let locked = kv::lock_shards(&self.txn, table_id, targets).await?;

            // 親が今もポインタノードで、子集合が降下時と同じであることを確かめる。
            // 違っていれば降下が古かったので、新しいスナップショットでやり直す。
            let child_regions = match locked.get(&parent_region) {
                Some(Some(value)) => match ShardEntry::child_pointers(value.entry())? {
                    Some(children) if children == descended_children => children,
                    _ => return Err(self.mark_stale().into()),
                },
                _ => return Err(self.mark_stale().into()),
            };

            // 子を走査し、いずれかがポインタノードならこのレベルは統合しない。
            // 全リーフで合算が閾値以下なら 1 リーフへ畳み込む。
            let mut combined = 0usize;
            let mut mergeable = true;
            for cr in &child_regions {
                // 空領域のキーはそもそも存在しないのでスキップ。
                let Some(Some(value)) = locked.get(cr) else {
                    continue;
                };
                match ShardEntry::leaf_count(value.entry())? {
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
            // バイト列はロック時に手元へ来ているので引き直さない。
            let mut child_maps: Vec<SpatialIdMap<Vec<u8>>> = Vec::new();
            for cr in &child_regions {
                let entry = locked.get(cr).and_then(|v| v.as_ref());
                let map = decode_leaf(cr, entry.map(ShardValue::entry))?;
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
                kv::delete_shard(&self.txn, table_id, &parent_region).await?;
            } else {
                self.put_leaf(table_id, &parent_region, &merged).await?;
            }
            for cr in &child_regions {
                kv::delete_shard(&self.txn, table_id, cr).await?;
            }

            // 親が新たなリーフになった → さらに上へ伝播。
            region = parent_region;
        }
        Ok(())
    }

    /// 制約変更時に、既存の格納値が新しい制約を満たすか検証する。
    ///
    /// テーブル全体をスナップショットで走査する。データ書き込みはテーブル単位の
    /// 排他を取らないので、これは**ある一点での検証**であり、検証中に走った書き込みは
    /// 対象に入らない。新しい制約はコミット後の書き込みには適用されるので、
    /// すり抜けうるのは「古い制約を読んだうえで検証後にコミットされた書き込み」だけ。
    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(super) async fn validate_existing_data(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        constraints: Option<&crate::models::database::table::TableConstraints>,
    ) -> Result<(), AppError> {
        let entries = kv::scan_shard_prefix(&self.txn, &keys::shards_of(table_id)).await?;
        for (_, value) in entries {
            let ShardEntry::Leaf(map_bytes) = ShardEntry::decode(value.entry())? else {
                continue;
            };
            // SAFETY: `decode_leaf` と同じ根拠（CRC 検証済みのバイト列）。
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
    keys::value_index(
        table_id,
        &value_index::order_preserving(data_type, value),
        flex_id,
    )
}
