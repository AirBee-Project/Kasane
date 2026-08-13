//! 葉の解決（CPU 側）。降下で決まったリーフから値を取り出して集約する。
//!
//! ネットワークには触れない。葉が多ければ rayon で分散する
//! （[`LEAF_PARALLEL_THRESHOLD`](super::LEAF_PARALLEL_THRESHOLD)）。

use kasane_logic::FlexId;
use rustc_hash::FxHashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::node::archived_leaf;
use super::routing::RoutedLeaf;
use super::{AppError, LEAF_PARALLEL_THRESHOLD, ValueMap};

// --- 葉の解決（CPU 側） ---

/// 1 枚のリーフを走査し、そこへ振り分けられた各クエリを解決して `by_value` へ積む。
///
/// FlexId ごとに値バイト列でハッシュすると、値の種類が少数でも結果 FlexId の数だけ
/// 長いバイト列をハッシュすることになる。そこでまず**この葉ローカルの辞書インデックス
/// （`u32`）**でグルーピングし（整数ハッシュは軽い）、葉に現れた distinct 値の数だけ
/// 実バイト列へ復元して全体マップへマージする。
pub(super) fn resolve_leaf(
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
pub(super) fn resolve_leaves(
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
pub(super) async fn resolve_leaves_off_worker(
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
