//! 木の降下。「どの flex_id / 範囲がどのリーフの担当か」を決める。
//!
//! ネットワークに触れるのはこの層と [`node`](super::node) だけで、
//! 上の [`resolve`](super::resolve) / [`leaf`](super::leaf) は純粋な計算になっている。

use kasane_logic::{FlexId, RangeId};
use rustc_hash::FxHashMap;
use std::sync::Arc;

use super::node::{decode_leaf, load_nodes};
use kasane_logic::SpatialIdMap;

use super::{AppError, Reader, Readers, ShardEntry, ShardValue, TableId};

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
    pub(super) fn leaf_map(&self) -> Result<SpatialIdMap<Vec<u8>>, AppError> {
        decode_leaf(&self.region, self.node.as_ref().map(ShardValue::entry))
    }
}

/// ある領域の親と、その親が持つ全子領域。
///
/// 兄弟のリストは親 1 つにつき 1 本しかないので、子の数だけ複製せず共有する。
pub(super) type Parentage = (FlexId, Arc<Vec<FlexId>>);

/// 子領域から親を引く対応表。
pub(super) type ParentMap = FxHashMap<FlexId, Parentage>;

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
pub(super) async fn route_leaves_batched<R: Reader>(
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
    pub node: ShardValue,
    /// このリーフへ到達した範囲の添字（呼び出し側が渡した `ranges` に対する）。
    pub hits: Vec<u32>,
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
pub(super) async fn route_leaves_for_ranges<R: Reader>(
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
