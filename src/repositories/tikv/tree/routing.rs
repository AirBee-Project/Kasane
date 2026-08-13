//! 木の降下。「どの flex_id / 範囲がどのリーフの担当か」を決める。
//! ネットワークに触れるのはこの層と [`node`](super::node) だけ。

use kasane_logic::{FlexId, RangeId};
use rustc_hash::FxHashMap;
use std::sync::Arc;

use super::node::load_nodes;
use super::{AppError, Reader, Readers, ShardEntry, ShardValue, TableId};

/// 降下の途中で読んだバイト列を持ち回るので、呼び出し側が同じキーを引き直さずに済む。
pub(super) struct RoutedLeaf {
    pub region: FlexId,
    /// ここへ到達した `flex_id` 群。
    pub queries: Vec<FlexId>,
    /// 未作成領域なら `None`。
    pub node: Option<ShardValue>,
}

/// 兄弟のリストは親 1 つにつき 1 本なので、子の数だけ複製せず [`Arc`] で共有する。
pub(super) type ParentMap = FxHashMap<FlexId, (FlexId, Arc<Vec<FlexId>>)>;

pub(super) struct Routing {
    /// 担当リーフ（未作成領域を含む）。
    pub leaves: Vec<RoutedLeaf>,
    /// 統合が必要とする対応。ルートから引き直すと単発 get が直列に積み上がる。
    pub parents: ParentMap,
}

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

/// 幅優先で同じ深さのノードをまとめて取得することで、往復を木の深さ分に抑える。
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
                    // バイト列はここで確定するので持たせておく（再取得しない）。
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

pub(super) struct RoutedRange {
    pub node: ShardValue,
    /// 呼び出し側が渡した `ranges` に対する添字。
    pub hits: Vec<u32>,
}

/// `ranges` のいずれかと重なる**既存のリーフ領域**を、木の降下 1 回で集める。
///
/// 範囲 1 本ごとにルートから降りると往復が**範囲の本数 × 木の深さ**になる。評価境界は対象
/// 空間 ID の FlexId ごとに 1 本ずつ立つので、この本数は要求の広さに比例して増える。
pub(super) async fn route_leaves_for_ranges<R: Reader>(
    txn: &Readers<R>,
    table_id: TableId,
    ranges: &[RangeId],
) -> Result<Vec<RoutedRange>, AppError> {
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
