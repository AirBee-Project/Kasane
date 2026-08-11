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
///
/// 戻ってきたキーからは [`keys::region_from_shard_key`] で領域を復元する。
/// キー → 領域の対応表を作る手もあるが、そうすると領域の数だけ
/// 「キーの複製」と「バイト列のハッシュ」が乗る。この関数は木の降下で**1 段につき
/// 1 回**呼ばれ、1 回で扱う領域数はテーブルが大きいほど増えるので、そこは削っておく。
async fn load_nodes<R: Reader>(
    txn: &Readers<R>,
    table_id: TableId,
    regions: &[FlexId],
) -> Result<FxHashMap<FlexId, ShardValue>, AppError> {
    let keys: Vec<Vec<u8>> = regions.iter().map(|r| keys::shard(table_id, r)).collect();
    kv::batch_get_shards(txn, keys)
        .await?
        .into_iter()
        .map(|(key, value)| Ok((keys::region_from_shard_key(&key)?, value)))
        .collect()
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

    /// クエリ実行器の入力として、指定範囲群の値をまとめて読み出す。
    ///
    /// 範囲を 1 本ずつ渡されないのが要点（[`route_leaves_for_ranges`] を参照）。
    ///
    /// **復元は葉を走査しながらその場で行う。** 格納バイト列を `Vec<u8>` として一旦
    /// 取り出すと、結果 1 行につきヒープ確保が 1 回起きる。呼び出し側はそれを直後に
    /// `V` へ復元して捨てるので、確保も複製も丸ごと無駄になる。件数はクエリの広さに
    /// 比例するため、この 1 行分が積み上がると効く（LMDB 側は借用のまま復元していて、
    /// もともとこの確保が無い）。
    #[tracing::instrument(skip_all, fields(table_id = %table_id, ranges = ranges.len()))]
    pub(super) async fn read_values_in_ranges<V: Send>(
        &self,
        table_id: TableId,
        ranges: &[RangeId],
        decode: &(dyn Fn(&[u8]) -> Option<V> + Send + Sync),
    ) -> Result<Vec<(FlexId, V)>, AppError> {
        if ranges.is_empty() {
            return Ok(Vec::new());
        }
        let leaves = route_leaves_for_ranges(&self.txn, table_id, ranges).await?;
        decode_range_leaves(&leaves, ranges, decode)
    }
}

/// 範囲に重なる `(FlexId, 値)` を葉から取り出し、その場で `V` へ復元する。
/// 葉が多ければ rayon で分散する。
///
/// ここは `Source::read_subset` の内側（＝クエリ実行器を回している blocking タスクの上）
/// から呼ばれるので、さらに blocking タスクへ出さずその場で並列化する。
///
/// 同じ `(FlexId, 値)` が複数の範囲から重複して出うるが、呼び出し側の union が
/// そのまま吸収する（`query_source.rs` の注記を参照）。復元できない値（型に合わない
/// 格納値）はここで落とす。
fn decode_range_leaves<V: Send>(
    leaves: &[RoutedRange],
    ranges: &[RangeId],
    decode: &(dyn Fn(&[u8]) -> Option<V> + Send + Sync),
) -> Result<Vec<(FlexId, V)>, AppError> {
    let decode_leaf = |leaf: &RoutedRange| -> Result<Vec<(FlexId, V)>, AppError> {
        let arch = archived_leaf(leaf.node.entry())?;
        let mut out = Vec::new();
        for &i in &leaf.hits {
            out.extend(
                arch.get_range(&ranges[i as usize])
                    .into_iter()
                    .filter_map(|(id, value)| decode(value).map(|value| (id, value))),
            );
        }
        Ok(out)
    };

    if leaves.len() < LEAF_PARALLEL_THRESHOLD {
        let mut out = Vec::new();
        for leaf in leaves {
            out.extend(decode_leaf(leaf)?);
        }
        return Ok(out);
    }

    use rayon::prelude::*;
    let parts: Vec<Vec<(FlexId, V)>> = leaves
        .par_iter()
        .map(&decode_leaf)
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
// # 楽観トランザクションにしない理由（実測）
//
// 上の不変条件は楽観でもそのまま通用する。「R は必ず書かれる」以上、構造的な競合は
// R というキーの競合として prewrite の検査に現れるからで、実際に一度そちらへ寄せた。
// 往復は `D+8` から `D+4` へ減り、降下で運んだリーフ本体を `batch_get_for_update` で
// もう一度運ぶ無駄も消える。
//
// **それでも悲観に戻した。** 楽観は競合の検出が prewrite まで遅れるので、1 回の競合で
// 捨てる仕事が「降下・復元・変更・シリアライズの全部」になる。同じ空間領域へ多数の
// 書き込みが集中する使い方（都市データの一括投入など）では、この差が決定的だった。
// トレースでは 6 割超の書き込みが 2 回以上やり直し、prewrite がロック解決の backoff
// （約 1.5 秒）を毎回使い切り、`Failed to resolve lock` が定常的に 500 として出た。
//
// 悲観ならロックの取得は着手前で、競合したその場で判る。捨てる仕事が小さいぶん
// やり直しが収束しやすい。**書き込みが落ちないことのほうが、往復数より優先される。**
//
// 低競合が前提の使い方なら楽観のほうが速い。戻すのであれば
// `super::write_options` を楽観へ変え、`lock_target_leaves` の
// `lock_shards` を素の読みへ差し替えればよい（`lock_options` は悲観のまま残すこと）。
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

    /// ロック済みのリーフ群へ変更を適用し、生じた変更をまとめて溜める。
    ///
    /// **計算は blocking タスクで回す。** リーフ 1 枚ごとに rkyv の復元と直列化が走る
    /// 重い処理で、非同期ワーカー上に置くとハートビートまで巻き添えにする
    /// （リーフの書き換えの節を参照）。ネットワークに触れるのはこの前後だけなので、
    /// 計算をまるごと外へ出せる。
    async fn apply_leaves(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        leaves: Vec<RoutedLeaf>,
        op: OwnedLeafOp,
    ) -> Result<(), AppError> {
        if leaves.is_empty() {
            return Ok(());
        }
        // blocking タスクは呼び出し元のスパンを引き継がないので、明示的に渡す。
        let span = tracing::Span::current();
        let mutations = tokio::task::spawn_blocking(move || {
            span.in_scope(|| {
                let mut out = Vec::new();
                for leaf in &leaves {
                    apply_leaf(table_id, index, leaf, &op.borrow(), &mut out)?;
                }
                Ok::<_, AppError>(out)
            })
        })
        .await
        .map_err(|e| AppError::InternalError(format!("leaf write task: {e}")))??;

        self.stage(mutations).await
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
        let leaves = self.lock_target_leaves(table_id, &flex_ids).await?.leaves;
        self.apply_leaves(table_id, index, leaves, OwnedLeafOp::Insert(data.to_vec()))
            .await
    }

    /// 値ごとに分かれた書き込みをまとめて適用する。
    ///
    /// **木の降下・ロック・リーフの書き直しをそれぞれ 1 回で済ませる。** 要素ごとに
    /// `data_insert_impl` を呼ぶと、要素の数だけ降下（深さぶんの往復）とロック取得が
    /// 直列に積み上がり、同じリーフを何度も復元・直列化し直すことになる。
    /// 全要素の空間 ID をまとめて振り分けてからリーフごとに適用すれば、コストは
    /// **触れたリーフの数**だけで決まる。
    #[tracing::instrument(skip_all, fields(table_id = %table_id, entries = entries.len()))]
    pub(super) async fn data_insert_many_impl(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        entries: Vec<(SpatialIdSet, Vec<u8>)>,
    ) -> Result<(), AppError> {
        let (batch, flex_ids) = BatchWrite::new(entries);
        if flex_ids.is_empty() {
            return Ok(());
        }
        let leaves = self.lock_target_leaves(table_id, &flex_ids).await?.leaves;
        self.apply_leaves(table_id, index, leaves, OwnedLeafOp::InsertMany(batch))
            .await
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
        let leaves = self.lock_target_leaves(table_id, &flex_ids).await?.leaves;
        self.apply_leaves(table_id, index, leaves, OwnedLeafOp::Upsert(data.to_vec()))
            .await
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

        let affected: Vec<FlexId> = routing.leaves.iter().map(|leaf| leaf.region).collect();
        self.apply_leaves(table_id, index, routing.leaves, OwnedLeafOp::Remove)
            .await?;

        // 同じ親を持つ兄弟がそれぞれ上へ辿ると、先の統合で消えた領域を後から調べに
        // 行くことになる。決着のついた領域を共有して 1 度で終わらせる。
        let mut settled = FxHashSet::default();
        for region in affected {
            self.try_merge_up(table_id, index, region, &routing.parents, &mut settled)
                .await?;
        }
        Ok(())
    }

    /// 溜めた変更をトランザクションへ渡す。
    async fn stage(&mut self, mutations: Vec<kv::Mutation>) -> Result<(), AppError> {
        kv::mutate_many(&self.txn, mutations).await
    }
}

// --- リーフの書き換え（純粋な計算） ---
//
// ここから下はネットワークに触れない。ロック済みのリーフのバイト列を受け取り、
// 適用すべき変更（[`kv::Mutation`]）を組み立てて返すだけである。
//
// 分けてあるのは、この計算が**重い**からである。リーフ 1 枚につき rkyv の復元
// （`SpatialIdMap::from_bytes` は `Arc` 木を丸ごと組み直す）と直列化が走り、
// 分割が起きればさらに増える。これを非同期ワーカー上で回すと、そのワーカーは
// その間まったく他のタスクを進められない。
//
// 巻き添えになるのは無関係なリクエストだけではない。**tikv-client が spawn した
// ハートビート**もワーカーを待つ。ハートビートはロックの寿命（20 秒）を 10 秒ごとに
// 延ばしているので、そこが遅れるとロックが期限切れになり、他者に回収される。
// 回収された側は失敗し、待っていた側は `Failed to resolve lock` を受け取る。
// つまり**CPU をワーカーへ置いたままにすると、負荷が上がったときに書き込みが落ちる**。
//
// 呼び出し側は [`TikvWrite::apply_leaves`] でここを blocking タスクへ出す。

/// 1 枚のリーフに何をするか。
enum LeafOp<'a> {
    /// 対象の空間 ID すべてへ書く（既存値は上書き）。
    Insert(&'a [u8]),
    /// まだ値の無い空間 ID にだけ書く。
    Upsert(&'a [u8]),
    /// 対象の空間 ID の値を消す。
    Remove,
    /// 空間 ID ごとに別々の値を書く（一括書き込み）。
    InsertMany(&'a BatchWrite),
}

/// 値ごとに分かれた一括書き込みを、リーフへ適用できる形に畳んだもの。
///
/// `owner` が `flex_id → values` の添字を持つので、リーフの走査中は**ハッシュ 1 回で
/// 書くべき値が決まる**。要素ごとに空間 ID 集合を持ち回って毎回交差を取ると、
/// 要素数 × リーフの FlexId 数になってしまう。
pub(super) struct BatchWrite {
    /// 同じ空間 ID が複数回指定された場合は**後勝ち**（後から挿入した添字が残る）。
    owner: FxHashMap<FlexId, u32>,
    values: Vec<Vec<u8>>,
}

impl BatchWrite {
    /// `(空間 ID, 値)` の並びから畳む。書き込み対象の全 `FlexId` も返す。
    fn new(entries: Vec<(SpatialIdSet, Vec<u8>)>) -> (Self, Vec<FlexId>) {
        let mut owner: FxHashMap<FlexId, u32> = FxHashMap::default();
        let mut values = Vec::with_capacity(entries.len());
        for (ids, value) in entries {
            let slot = values.len() as u32;
            values.push(value);
            for flex_id in ids.flex_ids() {
                // 後勝ち。1 件ずつ順に書いたときと同じ結果にする。
                owner.insert(flex_id, slot);
            }
        }
        let targets = owner.keys().copied().collect();
        (Self { owner, values }, targets)
    }
}

/// [`LeafOp`] の所有版。blocking タスクへ移すために借用を持たない形にしてある。
enum OwnedLeafOp {
    Insert(Vec<u8>),
    Upsert(Vec<u8>),
    Remove,
    InsertMany(BatchWrite),
}

impl OwnedLeafOp {
    fn borrow(&self) -> LeafOp<'_> {
        match self {
            OwnedLeafOp::Insert(data) => LeafOp::Insert(data),
            OwnedLeafOp::Upsert(data) => LeafOp::Upsert(data),
            OwnedLeafOp::Remove => LeafOp::Remove,
            OwnedLeafOp::InsertMany(batch) => LeafOp::InsertMany(batch),
        }
    }
}

impl LeafOp<'_> {
    fn apply(&self, map: &mut SpatialIdMap<Vec<u8>>, targets: &[FlexId]) {
        match self {
            LeafOp::Insert(data) => {
                for flex_id in targets {
                    map.insert(*flex_id, data.to_vec());
                }
            }
            LeafOp::Upsert(data) => {
                let mut target_set = SpatialIdSet::new();
                for flex_id in targets {
                    let occupied: SpatialIdSet = map.get(flex_id).map(|(f, _)| f).collect();
                    target_set.clear();
                    target_set.insert(*flex_id);
                    for f in (&target_set - &occupied).flex_ids() {
                        map.insert(f, data.to_vec());
                    }
                }
            }
            LeafOp::Remove => {
                for flex_id in targets {
                    map.remove(flex_id);
                }
            }
            LeafOp::InsertMany(batch) => {
                for flex_id in targets {
                    // 降下で振り分けられた ID は必ず台帳にある。無ければ他所の ID なので飛ばす。
                    if let Some(&slot) = batch.owner.get(flex_id) {
                        map.insert(*flex_id, batch.values[slot as usize].clone());
                    }
                }
            }
        }
    }
}

/// 1 つのリーフへの変更を適用し、値インデックスの差分と保存を変更として組み立てる。
///
/// `index` が `None`（索引を維持しないテーブル）なら、索引の差分計算そのものを飛ばす。
/// 索引キーは格納 `FlexId` 1 件につき 1 つ増えるので、ここを通るかどうかで
/// 1 回の書き込みが触るキー数が 3 桁変わる。
fn apply_leaf(
    table_id: TableId,
    index: Option<TableDataType>,
    leaf: &RoutedLeaf,
    op: &LeafOp<'_>,
    out: &mut Vec<kv::Mutation>,
) -> Result<(), AppError> {
    let mut map = leaf.leaf_map()?;
    let targets = &leaf.queries;

    let Some(data_type) = index else {
        op.apply(&mut map, targets);
        return store_shard(table_id, leaf.region, map, out);
    };

    let scan: SpatialIdSet = targets.iter().cloned().collect();

    // 変更前の重なりリーフからインデックスキーを計算。
    let mut old_keys = FxHashSet::default();
    let mut pre_modify_scan = scan.clone();
    for f_scan in scan.iter() {
        for (f, v) in map.get_overlapping(&f_scan) {
            old_keys.insert(index_key(table_id, data_type, v, &f));
            pre_modify_scan.insert(f);
        }
    }

    op.apply(&mut map, targets);

    // 変更後の重なりリーフからインデックスキーを計算。
    let mut new_keys = FxHashSet::default();
    for f_scan in pre_modify_scan.iter() {
        for (f, v) in map.get_overlapping(&f_scan) {
            new_keys.insert(index_key(table_id, data_type, v, &f));
        }
    }

    kv::value_index_mutations(&old_keys, &new_keys, out);
    store_shard(table_id, leaf.region, map, out)
}

/// 変更後のリーフを保存する。過大なら分割し、空なら削除する。
fn store_shard(
    table_id: TableId,
    region: FlexId,
    map: SpatialIdMap<Vec<u8>>,
    out: &mut Vec<kv::Mutation>,
) -> Result<(), AppError> {
    if !map.should_split_shard(MAX_FLEX_ID_PER_SHARD) {
        if map.is_empty() {
            out.extend(kv::shard_deletions(table_id, &region));
        } else {
            put_leaf(table_id, &region, &map, out)?;
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
    emit_child(table_id, lo_r, lo, &mut children, out)?;
    emit_child(table_id, hi_r, hi, &mut children, out)?;

    out.extend(kv::shard_mutations(
        table_id,
        &region,
        &ShardEntry::encode_pointers(&children),
    )?);
    Ok(())
}

/// 分割された子シャードを保存するか、さらに分割するかを決める（パス圧縮の本体）。
///
/// 同期関数なので普通に再帰できる（非同期だった頃は `Box::pin` で間接化していた）。
fn emit_child(
    table_id: TableId,
    cr: FlexId,
    cm: SpatialIdMap<Vec<u8>>,
    covers: &mut Vec<FlexId>,
    out: &mut Vec<kv::Mutation>,
) -> Result<(), AppError> {
    if cm.is_empty() {
        // 空領域：被覆として領域だけ積む。万一の古いキーは消す。
        out.extend(kv::shard_deletions(table_id, &cr));
        covers.push(cr);
        return Ok(());
    }
    if !cm.should_split_shard(MAX_FLEX_ID_PER_SHARD) {
        put_leaf(table_id, &cr, &cm, out)?;
        covers.push(cr);
        return Ok(());
    }

    // 過大：1 段だけ覗いて、退化分割か実分割かを決める。
    let ((clo_r, clo), (chi_r, chi)) = cm
        .split_shard()
        .ok_or_else(|| AppError::InternalError("split on shardless map".to_string()))?;

    if clo.is_empty() || chi.is_empty() {
        // 退化分割：中間ポインタを作らず孫を巻き上げる（チェーン圧縮）。
        emit_child(table_id, clo_r, clo, covers, out)?;
        emit_child(table_id, chi_r, chi, covers, out)?;
    } else {
        // 実分割：cr を独立ポインタノードにする。
        let mut grand = Vec::new();
        emit_child(table_id, clo_r, clo, &mut grand, out)?;
        emit_child(table_id, chi_r, chi, &mut grand, out)?;
        out.extend(kv::shard_mutations(
            table_id,
            &cr,
            &ShardEntry::encode_pointers(&grand),
        )?);
        covers.push(cr);
    }
    Ok(())
}

/// リーフを件数ヘッダ付きで保存する。
fn put_leaf(
    table_id: TableId,
    region: &FlexId,
    map: &SpatialIdMap<Vec<u8>>,
    out: &mut Vec<kv::Mutation>,
) -> Result<(), AppError> {
    let bytes = map
        .to_bytes()
        .map_err(|e| AppError::InternalError(format!("rkyv serialize: {e}")))?;
    out.extend(kv::shard_mutations(
        table_id,
        region,
        &ShardEntry::encode_leaf(map.count() as u32, &bytes),
    )?);
    Ok(())
}

/// 親へ畳んだ結果を変更として組み立てる（[`TikvWrite::try_merge_up`] の重い部分）。
///
/// 子のバイト列はロック時に手元へ来ているので引き直さない。
fn merge_children(
    table_id: TableId,
    index: Option<TableDataType>,
    parent_region: FlexId,
    child_regions: &[FlexId],
    locked: &std::collections::BTreeMap<FlexId, Option<ShardValue>>,
) -> Result<Vec<kv::Mutation>, AppError> {
    let mut child_maps: Vec<SpatialIdMap<Vec<u8>>> = Vec::new();
    for cr in child_regions {
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

    let mut out = Vec::new();
    if let (Some(data_type), Some(old_keys)) = (index, old_keys) {
        let mut new_keys = FxHashSet::default();
        for (f, v) in merged.iter() {
            new_keys.insert(index_key(table_id, data_type, v, &f));
        }
        kv::value_index_mutations(&old_keys, &new_keys, &mut out);
    }

    // 親キーをリーフ（空なら削除）に置換し、子キーを削除する。
    if merged.is_empty() {
        out.extend(kv::shard_deletions(table_id, &parent_region));
    } else {
        put_leaf(table_id, &parent_region, &merged, &mut out)?;
    }
    for cr in child_regions {
        out.extend(kv::shard_deletions(table_id, cr));
    }
    Ok(out)
}

impl TikvWrite<'_> {
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

            // 親と全子をロックし、ロック時点の内容を得る。子には**このループで既に
            // 畳んだもの**が含まれるので、自分の変更が重なった状態で返る必要がある
            // （`kv::lock_shards` を参照）。
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

            // ここから先は重い（子マップの復元・統合・直列化）ので blocking タスクへ出す。
            // 非同期ワーカー上に置くとハートビートを巻き添えにする
            // （リーフの書き換えの節を参照）。
            let span = tracing::Span::current();
            let regions = child_regions.clone();
            let mutations = tokio::task::spawn_blocking(move || {
                span.in_scope(|| merge_children(table_id, index, parent_region, &regions, &locked))
            })
            .await
            .map_err(|e| AppError::InternalError(format!("shard merge task: {e}")))??;

            self.stage(mutations).await?;

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
