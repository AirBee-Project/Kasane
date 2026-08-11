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
//! - 受信バッファは信用できないので、rkyv の非検証アクセスへ渡す前に
//!   [`kv::ShardValue`] の完全性検証を通す（`kv.rs` のフレームの節を参照）。
//! - 再帰は `Box::pin` で明示的に間接化する（async fn の再帰のため）。
//!
//! # 読み取りのゼロコピー
//!
//! 読み取り経路は受信バッファを [`ArchivedSpatialIdMap`] で**直接走査**する。LMDB 側が
//! mmap 上でやっているのと同じことを、mmap の代わりに受信バッファに対して行うだけ。
//!
//! `SpatialIdMap::from_bytes` を通すと `Arc` ベースの作業木を丸ごと組み直すことになり、
//! リーフ 1 枚（最大 [`MAX_FLEX_ID_PER_SHARD`] 件）につき数千回のノード確保と、葉ごとの
//! 値バイト列の複製、保存していない導出値の畳み直しが走る。そのどれも読むだけなら要らない。
//! 復元が要るのは**書き換える**ときだけなので、`from_bytes` は書き込み経路に残してある。
//!
//! # CPU をどこで回すか
//!
//! リーフの走査と集約は FlexId 数に比例する CPU 処理で、TiKV バックエンドではこれが
//! 非同期ワーカー上で回る（LMDB はクロージャ全体が blocking タスク上なので問題にならない）。
//! 大きな検索がワーカーを占有すると無関係なリクエストまで止まるため、ルーティング
//! （ネットワーク）と解決（CPU）を分け、後者を blocking タスクへ出したうえで葉が多ければ
//! rayon で分散する。

use kasane_logic::{ArchivedSpatialIdMap, FlexId, RangeId, SpatialIdMap, SpatialIdSet};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::keys;
use crate::error::AppError;
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::repositories::ValueGroups;
use crate::repositories::encoding::shard_entry::{
    MAX_FLEX_ID_PER_SHARD, MERGE_FLEX_ID_THRESHOLD, ShardEntry,
};
use crate::repositories::encoding::value_index;

use super::kv::{Reader, Readers, ShardValue};
use super::{TikvRead, TikvWrite, kv};

/// 値ごとに FlexId をまとめた中間表現。
type ValueMap = FxHashMap<Vec<u8>, Vec<FlexId>>;

/// 葉の解決を rayon へ出す基準（触れる**リーフ数**）。
///
/// 基準を「クエリ FlexId 数」ではなく「実際に触れる葉の数」に置くのは LMDB 側と同じ理由で、
/// 広域検索はクエリ FlexId が数個でも数千の葉に及ぶため。
const LEAF_PARALLEL_THRESHOLD: usize = 32;

// --- ノードの読み書き ---

/// 複数領域のノードをまとめて取得する。存在しない領域は結果に含まれない。
///
/// 呼び出し側はキーではなく領域で引きたいので、領域をキーにして返す
/// （キーで返すと、引くたびにキーを組み立て直すことになる）。
async fn load_nodes<R: Reader>(
    txn: &Readers<R>,
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
///
/// **書き換えるとき専用。** 読むだけなら [`archived_leaf`] を使う。
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

/// リーフのバイト列をゼロコピーで開く（読み取り専用）。
fn archived_leaf(entry: &[u8]) -> Result<ArchivedSpatialIdMap<'_>, AppError> {
    match ShardEntry::leaf_payload(entry)? {
        // SAFETY: `decode_leaf` と同じ根拠（CRC 検証済みのバイト列）。形式バージョンは
        // `access` 側でさらに検証される。
        Some(map_bytes) => unsafe { ArchivedSpatialIdMap::access(map_bytes) }
            .map_err(|e| AppError::InternalError(format!("leaf format: {e}"))),
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

/// ある領域の親と、その親が持つ全子領域。
///
/// 兄弟のリストは親 1 つにつき 1 本しかないので、子の数だけ複製せず共有する。
type Parentage = (FlexId, Arc<Vec<FlexId>>);

/// 子領域から親を引く対応表。
type ParentMap = FxHashMap<FlexId, Parentage>;

/// 降下でわかった木の形。
pub(super) struct Routing {
    /// 担当リーフ（未作成領域を含む）。
    pub leaves: Vec<RoutedLeaf>,
    /// 通り道で見た「子 → (親, 親の全子)」の対応。
    ///
    /// 統合（[`TikvWrite::try_merge_up`]）はこの対応を必要とするが、それは降下の途中で
    /// すでに判っている。改めてルートから引き直すと、**影響リーフ数 × 木の深さ**ぶんの
    /// 単発 get が直列に積み上がる（祖先は共通なのに毎回引き直すことになる）。
    pub parents: ParentMap,
}

/// 複数の `flex_id` を木の降下でまとめて振り分ける。
///
/// 書き込み経路でも使うため、まだノードが作られていない領域も担当リーフとして返す。
async fn route_leaves_batched<R: Reader>(
    txn: &Readers<R>,
    table_id: TableId,
    ids: &[FlexId],
) -> Result<Routing, AppError> {
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
    let mut parents = ParentMap::default();
    descend_batched(
        txn,
        table_id,
        FlexId::LOWER_MAX,
        lower,
        &mut out,
        &mut parents,
    )
    .await?;
    descend_batched(
        txn,
        table_id,
        FlexId::UPPER_MAX,
        upper,
        &mut out,
        &mut parents,
    )
    .await?;
    Ok(Routing {
        leaves: out.into_values().collect(),
        parents,
    })
}

/// `region` を根として `ids` を子へ振り分けながら降りる。
///
/// 幅優先で「同じ深さのノードをまとめて取得」してから振り分けることで、
/// ネットワーク往復を木の深さ分に抑える。
async fn descend_batched<R: Reader>(
    txn: &Readers<R>,
    table_id: TableId,
    root: FlexId,
    ids: Vec<FlexId>,
    out: &mut FxHashMap<FlexId, RoutedLeaf>,
    parents: &mut ParentMap,
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
                    // 降りない子も含めて対応を残す。統合はここで判った形をそのまま使う。
                    let siblings = Arc::new(children);
                    for child in siblings.iter() {
                        parents.insert(*child, (region, siblings.clone()));
                        let sub: Vec<FlexId> = bucket
                            .iter()
                            .filter(|f| child.intersection(f).is_some())
                            .copied()
                            .collect();
                        if !sub.is_empty() {
                            next.push((*child, sub));
                        }
                    }
                }
            }
        }
        level = next;
    }

    Ok(())
}

/// 範囲群が到達したリーフ。
pub(super) struct RoutedRange {
    /// リーフのバイト列。
    node: ShardValue,
    /// このリーフへ到達した範囲の添字（呼び出し側が渡した `ranges` に対する）。
    hits: Vec<u32>,
}

/// `ranges` のいずれかと重なる**既存のリーフ領域**を、木の降下 1 回で集める。
///
/// 範囲 1 本ごとにルートから降りると、往復は**範囲の本数 × 木の深さ**になる。
/// クエリの評価境界は対象空間 ID セットの FlexId ごとに 1 本ずつ立つので、この本数は
/// 要求の広さに比例して増える。[`descend_batched`] が FlexId に対してやっているのと
/// 同じように、同じ深さのノードをまとめて取得しながら範囲を子へ振り分ければ、
/// 往復は木の深さぶんに収まる。
///
/// 重なり合う範囲が同じリーフへ到達しても、リーフの取得は 1 回きりになる
/// （以前は範囲の本数だけ同じバイト列を転送していた）。
async fn route_leaves_for_ranges<R: Reader>(
    txn: &Readers<R>,
    table_id: TableId,
    ranges: &[RangeId],
) -> Result<Vec<RoutedRange>, AppError> {
    // (領域, そこへ到達した範囲の添字) を深さごとに処理する。
    let mut level: Vec<(FlexId, Vec<u32>)> = Vec::new();
    for root in [FlexId::LOWER_MAX, FlexId::UPPER_MAX] {
        let hits: Vec<u32> = ranges
            .iter()
            .enumerate()
            .filter(|(_, range)| root.intersects_range(range))
            .map(|(i, _)| i as u32)
            .collect();
        if !hits.is_empty() {
            level.push((root, hits));
        }
    }

    let mut out = Vec::new();
    while !level.is_empty() {
        let regions: Vec<FlexId> = level.iter().map(|(region, _)| *region).collect();
        let mut nodes = load_nodes(txn, table_id, &regions).await?;

        let mut next: Vec<(FlexId, Vec<u32>)> = Vec::new();
        for (region, hits) in level {
            // 未作成領域＝データ無し。読み取りでは辿る必要がない。
            let Some(value) = nodes.remove(&region) else {
                continue;
            };
            match ShardEntry::child_pointers(value.entry())? {
                // 読んだバイト列をそのまま返し、呼び出し側の再取得をなくす。
                None => out.push(RoutedRange { node: value, hits }),
                Some(children) => {
                    for child in children {
                        let sub: Vec<u32> = hits
                            .iter()
                            .copied()
                            .filter(|&i| child.intersects_range(&ranges[i as usize]))
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

    Ok(out)
}

// --- 葉の解決（CPU 側） ---

/// 1 枚のリーフを走査し、そこへ振り分けられた各クエリを解決して `by_value` へ積む。
///
/// FlexId ごとに値バイト列でハッシュすると、値の種類が少数でも結果 FlexId の数だけ
/// 長いバイト列をハッシュすることになる。そこでまず**この葉ローカルの辞書インデックス
/// （`u32`）**でグルーピングし（整数ハッシュは軽い）、葉に現れた distinct 値の数だけ
/// 実バイト列へ復元して全体マップへマージする。
fn resolve_leaf(
    leaf: &RoutedLeaf,
    by_value: &mut ValueMap,
    limit: Option<usize>,
    counter: &AtomicUsize,
) -> Result<(), AppError> {
    // 未作成領域にはデータが無い。
    let Some(node) = &leaf.node else {
        return Ok(());
    };
    let arch = archived_leaf(node.entry())?;

    let mut local: FxHashMap<u32, Vec<FlexId>> = FxHashMap::default();
    // カウンタへの反映をまとめる粒度。1 件ごとに触ると原子操作が支配的になる。
    let batch = limit.unwrap_or(0).clamp(1, 256);
    let mut counted = 0usize;

    for query in &leaf.queries {
        if limit.is_some_and(|l| counter.load(Ordering::Relaxed) >= l) {
            break;
        }
        arch.get_indexed(query, |got, packed| {
            if let Some(limit) = limit {
                if counter.load(Ordering::Relaxed) >= limit {
                    return;
                }
                counted += 1;
                if counted >= batch {
                    counter.fetch_add(counted, Ordering::Relaxed);
                    counted = 0;
                }
            }
            local.entry(packed).or_default().push(got);
        });
    }
    if counted > 0 {
        counter.fetch_add(counted, Ordering::Relaxed);
    }

    // 葉の distinct 値だけ実バイト列へ復元して全体マップへマージする。
    for (packed, mut flex_ids) in local {
        let value = arch.value_bytes(packed);
        match by_value.get_mut(value) {
            Some(existing) => existing.append(&mut flex_ids),
            None => {
                by_value.insert(value.to_vec(), flex_ids);
            }
        }
    }
    Ok(())
}

/// 葉をまとめて解決する。葉が多ければ rayon で分散する。
///
/// 葉は互いに独立（同一 FlexId は 1 つの葉にしか属さない）なので、部分マップを作って
/// 最後に値でマージするだけで正しく合流できる。
fn resolve_leaves(
    leaves: &[RoutedLeaf],
    limit: Option<usize>,
    held: usize,
) -> Result<ValueMap, AppError> {
    let counter = AtomicUsize::new(held);

    let resolve_chunk = |chunk: &[RoutedLeaf]| -> Result<ValueMap, AppError> {
        let mut out = ValueMap::default();
        for leaf in chunk {
            if limit.is_some_and(|l| counter.load(Ordering::Relaxed) >= l) {
                break;
            }
            resolve_leaf(leaf, &mut out, limit, &counter)?;
        }
        Ok(out)
    };

    if leaves.len() < LEAF_PARALLEL_THRESHOLD {
        return resolve_chunk(leaves);
    }

    use rayon::prelude::*;
    // 葉ごとに 1 タスクにすると、小さな葉ばかりのときに分配のほうが高くつく。
    let chunk = leaves
        .len()
        .div_ceil(rayon::current_num_threads().max(1))
        .max(1);
    let partials: Vec<ValueMap> = leaves
        .par_chunks(chunk)
        .map(&resolve_chunk)
        .collect::<Result<Vec<_>, AppError>>()?;

    let mut out = ValueMap::default();
    for partial in partials {
        for (value, mut flex_ids) in partial {
            match out.get_mut(&value) {
                Some(existing) => existing.append(&mut flex_ids),
                None => {
                    out.insert(value, flex_ids);
                }
            }
        }
    }
    Ok(out)
}

/// [`resolve_leaves`] を blocking タスクで回す。
///
/// TiKV バックエンドの読み取りは非同期ワーカー上で走るので、大きな結果の CPU 処理を
/// そのまま置くとワーカーを占有し、無関係なリクエストまで止まる（モジュール冒頭を参照）。
async fn resolve_leaves_off_worker(
    leaves: Vec<RoutedLeaf>,
    limit: Option<usize>,
    held: usize,
) -> Result<ValueMap, AppError> {
    if leaves.is_empty() {
        return Ok(ValueMap::default());
    }
    // blocking タスクは呼び出し元のスパンを引き継がないので、明示的に渡す。
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || span.in_scope(|| resolve_leaves(&leaves, limit, held)))
        .await
        .map_err(|e| AppError::InternalError(format!("leaf resolution task: {e}")))?
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
        let mut by_value = ValueMap::default();
        let mut held = 0usize;

        // 全件をまとめてルーティングせず、チャンクに区切って `limit` へ達した時点で
        // 打ち切る。ここで区切るのは**打ち切りの粒度**と手元に持つ `FlexId` の量で、
        // 1 リクエストのキー数ではない（そちらは `kv::BATCH_KEYS` が別途縛る。
        // 木の同じ深さで引くキー数は相異なる領域の数であって、`flex_id` の数ではない）。
        const ROUTING_BATCH_SIZE: usize = 8192;
        let mut iter = ids.flex_ids();

        loop {
            if limit.is_some_and(|l| held >= l) {
                break;
            }
            let batch: Vec<FlexId> = iter.by_ref().take(ROUTING_BATCH_SIZE).collect();
            if batch.is_empty() {
                break;
            }

            // ルーティング（ネットワーク）と解決（CPU）を分ける。前者はここで、
            // 後者は blocking タスクの上で葉ごとに並列に回す。
            let routed = route_leaves_batched(&self.txn, table_id, &batch)
                .await?
                .leaves;
            let partial = resolve_leaves_off_worker(routed, limit, held).await?;

            for (value, mut flex_ids) in partial {
                held += flex_ids.len();
                match by_value.get_mut(&value) {
                    Some(existing) => existing.append(&mut flex_ids),
                    None => {
                        by_value.insert(value, flex_ids);
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

    /// クエリ実行器の入力として、指定範囲群の FlexId をまとめて読み出す。
    ///
    /// 範囲を 1 本ずつ渡されないのが要点（[`route_leaves_for_ranges`] を参照）。
    #[tracing::instrument(skip_all, fields(table_id = %table_id, ranges = ranges.len()))]
    pub(super) async fn read_flex_ids_in_ranges(
        &self,
        table_id: TableId,
        ranges: &[RangeId],
    ) -> Result<Vec<(FlexId, Vec<u8>)>, AppError> {
        if ranges.is_empty() {
            return Ok(Vec::new());
        }
        let leaves = route_leaves_for_ranges(&self.txn, table_id, ranges).await?;
        decode_range_leaves(&leaves, ranges)
    }
}

/// 範囲に重なる `(FlexId, 値)` を葉から取り出す。葉が多ければ rayon で分散する。
///
/// ここは `Source::read_subset` の内側（＝クエリ実行器を回している blocking タスクの上）
/// から呼ばれるので、さらに blocking タスクへ出さずその場で並列化する。
///
/// 同じ `(FlexId, 値)` が複数の範囲から重複して出うるが、呼び出し側の union が
/// そのまま吸収する（`query_source.rs` の注記を参照）。
fn decode_range_leaves(
    leaves: &[RoutedRange],
    ranges: &[RangeId],
) -> Result<Vec<(FlexId, Vec<u8>)>, AppError> {
    let decode = |leaf: &RoutedRange| -> Result<Vec<(FlexId, Vec<u8>)>, AppError> {
        let arch = archived_leaf(leaf.node.entry())?;
        let mut out = Vec::new();
        for &i in &leaf.hits {
            out.extend(
                arch.get_range(&ranges[i as usize])
                    .into_iter()
                    .map(|(id, value)| (id, value.to_vec())),
            );
        }
        Ok(out)
    };

    if leaves.len() < LEAF_PARALLEL_THRESHOLD {
        let mut out = Vec::new();
        for leaf in leaves {
            out.extend(decode(leaf)?);
        }
        return Ok(out);
    }

    use rayon::prelude::*;
    let parts: Vec<Vec<(FlexId, Vec<u8>)>> = leaves
        .par_iter()
        .map(&decode)
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(parts.into_iter().flatten().collect())
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
    ) -> Result<Routing, AppError> {
        let mut routed = route_leaves_batched(&self.txn, table_id, flex_ids).await?;
        let regions: BTreeSet<FlexId> = routed.leaves.iter().map(|leaf| leaf.region).collect();
        let mut locked = kv::lock_shards(&self.txn, table_id, regions).await?;

        for leaf in &mut routed.leaves {
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
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();
        for leaf in self.lock_target_leaves(table_id, &flex_ids).await?.leaves {
            let map = leaf.leaf_map()?;
            let targets = leaf.queries;
            self.apply_leaf(table_id, index, leaf.region, map, &targets, |m| {
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
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();
        let data_vec = data.to_vec();
        for leaf in self.lock_target_leaves(table_id, &flex_ids).await?.leaves {
            let map = leaf.leaf_map()?;
            let targets = leaf.queries;
            self.apply_leaf(table_id, index, leaf.region, map, &targets, |m| {
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
        index: Option<TableDataType>,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();
        let routing = self.lock_target_leaves(table_id, &flex_ids).await?;

        let mut affected: Vec<FlexId> = Vec::new();
        for leaf in routing.leaves {
            let map = leaf.leaf_map()?;
            let targets = leaf.queries;
            let region = leaf.region;
            self.apply_leaf(table_id, index, region, map, &targets, |m| {
                for flex_id in &targets {
                    m.remove(flex_id);
                }
            })
            .await?;
            affected.push(region);
        }

        // 同じ親を持つ兄弟がそれぞれ上へ辿ると、先の統合で消えた領域を後から調べに
        // 行くことになる。決着のついた領域を共有して 1 度で終わらせる。
        let mut settled = FxHashSet::default();
        for region in affected {
            self.try_merge_up(table_id, index, region, &routing.parents, &mut settled)
                .await?;
        }
        Ok(())
    }

    /// 1 つのリーフへの変更を適用し、値インデックスを差分更新してから保存する。
    ///
    /// `index` が `None`（索引を維持しないテーブル）なら、索引の差分計算そのものを飛ばす。
    /// 索引キーは格納 `FlexId` 1 件につき 1 つ増えるので、ここを通るかどうかで
    /// 1 回の書き込みが触るキー数が 3 桁変わる。
    async fn apply_leaf<F>(
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
        let Some(data_type) = index else {
            modify(&mut map);
            return self.store_shard(table_id, region, map).await;
        };

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
    ///
    /// 親子関係は降下で判ったもの（`parents`）をそのまま使い、ルートから引き直さない。
    /// 引き直しても同じ `start_ts` を読むので答えは変わらず、下の検証が「降下時と同じか」
    /// を確かめる以上、根拠として過不足がない。
    ///
    /// `settled` は決着のついた領域（畳まれて消えた、あるいは畳めないと判った）を
    /// 呼び出し間で共有する。これが無いと、兄弟ごとに同じ親を調べ直したり、
    /// 自分で消した領域を調べに行って `mark_stale` で無駄にやり直したりする。
    async fn try_merge_up(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        region: FlexId,
        parents: &ParentMap,
        settled: &mut FxHashSet<FlexId>,
    ) -> Result<(), AppError> {
        let mut region = region;
        loop {
            if settled.contains(&region) {
                break;
            }
            // 対応が無いのはルートに達したとき（ルート自身に親はない）。
            let Some((parent_region, descended_children)) = parents.get(&region) else {
                break;
            };
            let parent_region = *parent_region;

            // 親と全子をロックし、ロック時点の内容を得る。
            let mut targets: BTreeSet<FlexId> = descended_children.iter().copied().collect();
            targets.insert(parent_region);
            let locked = kv::lock_shards(&self.txn, table_id, targets).await?;

            // 親が今もポインタノードで、子集合が降下時と同じであることを確かめる。
            // 違っていれば降下が古かったので、新しいスナップショットでやり直す。
            let child_regions = match locked.get(&parent_region) {
                Some(Some(value)) => match ShardEntry::child_pointers(value.entry())? {
                    Some(children) if children == **descended_children => children,
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
                // この親はもう畳めない。兄弟が同じ判定を繰り返しても答えは変わらない。
                settled.extend(child_regions);
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

            // 索引を維持しないテーブルでは、統合は木の形を変えるだけ。`(FlexId, 値)` の
            // 対応は親へ移っても変わらないので、索引の差分計算そのものが要らない。
            let old_keys = index.map(|data_type| {
                let mut keys = FxHashSet::default();
                for m in &child_maps {
                    for (f, v) in m.iter() {
                        keys.insert(index_key(table_id, data_type, v, &f));
                    }
                }
                keys
            });

            let merged = SpatialIdMap::merge_shards(parent_region, child_maps)?;

            if let (Some(data_type), Some(old_keys)) = (index, old_keys) {
                let mut new_keys = FxHashSet::default();
                for (f, v) in merged.iter() {
                    new_keys.insert(index_key(table_id, data_type, v, &f));
                }
                self.update_value_index(old_keys, new_keys).await?;
            }

            // 親キーをリーフ（空なら削除）に置換し、子キーを削除する。
            if merged.is_empty() {
                kv::delete_shard(&self.txn, table_id, &parent_region).await?;
            } else {
                self.put_leaf(table_id, &parent_region, &merged).await?;
            }
            for cr in &child_regions {
                kv::delete_shard(&self.txn, table_id, cr).await?;
            }

            // 子はもう存在しない。兄弟がここから上を辿り直さないよう印を付ける。
            settled.extend(child_regions);

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
            // ポインタノードには値が無い。
            if ShardEntry::leaf_payload(value.entry())?.is_none() {
                continue;
            }
            let arch = archived_leaf(value.entry())?;
            // 全域を覆う範囲で舐める。検証したいのは格納値そのものなので、
            // 葉を作業木へ組み直す必要はない。
            for range in [FlexId::LOWER_MAX, FlexId::UPPER_MAX].map(RangeId::from) {
                for (_, stored) in arch.get_range(&range) {
                    let restored = crate::services::helpers::value::restore_value(
                        data_type,
                        constraints,
                        stored,
                    )?;
                    crate::services::helpers::value::interpret_value(
                        data_type,
                        constraints,
                        restored,
                    )?;
                }
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
