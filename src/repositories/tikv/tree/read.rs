//! 読み取りの入口。`TikvRead` に生やす固有メソッドはここだけ。
//! 降下（ネットワーク）は [`routing`](super::routing)、葉の解決（CPU）はここが持つ。

use std::sync::atomic::{AtomicUsize, Ordering};

use kasane_logic::{FlexId, RangeId, SpatialIdSet};
use rustc_hash::FxHashMap;
use tracing::Instrument;

use super::node::archived_leaf;
use super::routing::{RoutedLeaf, RoutedRange, route_leaves_batched, route_leaves_for_ranges};
use super::{
    AppError, LEAF_PARALLEL_THRESHOLD, NodeCache, Reader, TableDataType, TableId, TikvRead,
    ValueMap, keys, kv,
};
use crate::repositories::ValueGroups;
use crate::repositories::encoding::value_index;

// --- 読み取り ---

impl<R: Reader> TikvRead<'_, R> {
    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(in crate::repositories::tikv) async fn data_get_impl(
        &self,
        table_id: TableId,
        ids: SpatialIdSet,
        limit: Option<usize>,
    ) -> Result<ValueGroups, AppError> {
        let mut by_value = ValueMap::default();
        let mut held = 0usize;

        // 区切るのは打ち切りの粒度であって、1 リクエストのキー数ではない（`kv::BATCH_KEYS`）。
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

            let routed = route_leaves_batched(&self.txn, table_id, &batch)
                .await?
                .leaves;
            let partial = resolve_leaves(routed, limit, held).await?;

            for (value, flex_ids) in partial {
                held += flex_ids.len();
                merge_into(&mut by_value, &value, flex_ids);
            }
        }

        Ok(by_value.into_iter().collect())
    }

    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(in crate::repositories::tikv) async fn data_filter_eq_impl(
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
            // 前方一致しただけの別キーを除外（残りがちょうど flexid 分の長さ）。
            if key.len() != prefix.len() + FlexId::ENCODED_LEN {
                continue;
            }
            out.push(value_index::flexid_from_key(&key[1..])?);
        }
        Ok(out)
    }

    /// 値が `lo`〜`hi`（両端含む）に入る FlexId を引く。
    ///
    /// 可変長型はバイト範囲だけで絞りきれないので、型の幅で読む範囲を決めてから
    /// 取り出した `vkey` で厳密に絞る（`encoding::value_index::range_scan_bounds`）。
    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(in crate::repositories::tikv) async fn data_filter_range_impl(
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
            // 覆う最小の範囲まで絞り、あとは下の厳密フィルタに任せる。
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

    /// **復元は葉を走査しながらその場で行う。**
    ///
    /// 格納バイト列を `Vec<u8>` として一旦取り出すと結果 1 行につきヒープ確保が 1 回起き、
    /// 呼び出し側はそれを直後に `V` へ復元して捨てるので丸ごと無駄になる。
    #[tracing::instrument(skip_all, fields(table_id = %table_id, ranges = ranges.len()))]
    pub(in crate::repositories::tikv) async fn read_values_in_ranges<V: Send>(
        &self,
        table_id: TableId,
        ranges: &[RangeId],
        decode: &(dyn Fn(&[u8]) -> Option<V> + Send + Sync),
        node_cache: &NodeCache,
    ) -> Result<Vec<(FlexId, V)>, AppError> {
        if ranges.is_empty() {
            return Ok(Vec::new());
        }

        // ネットワーク降下と CPU 復号のどちらが支配的かを切り分けるための内訳。
        let leaves = route_leaves_for_ranges(&self.txn, table_id, ranges, node_cache)
            .instrument(tracing::info_span!(
                "tikv.route_leaves",
                ranges = ranges.len()
            ))
            .await?;

        let decode_span = tracing::info_span!("tikv.decode_range_leaves", leaves = leaves.len());
        let _guard = decode_span.enter();
        decode_range_leaves(&leaves, ranges, decode)
    }
}

/// 1 枚のリーフを走査し、`by_value` へ積む。
///
/// 葉ローカルの辞書インデックス（`u32`）で先にグルーピングするのは、値バイト列で直接ハッシュ
/// すると結果 FlexId の数だけ長いバイト列をハッシュすることになるため。
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

    // 葉に現れた distinct 値の数だけ実バイト列へ復元する。
    for (packed, flex_ids) in local {
        merge_into(by_value, arch.value_bytes(packed), flex_ids);
    }
    Ok(())
}

/// 部分マップを作って最後にマージできるのは、同一 FlexId が 1 つの葉にしか属さないため。
///
/// 走査は非同期ワーカーを占有しないよう blocking タスクの上で行う。
async fn resolve_leaves(
    leaves: Vec<RoutedLeaf>,
    limit: Option<usize>,
    held: usize,
) -> Result<ValueMap, AppError> {
    if leaves.is_empty() {
        return Ok(ValueMap::default());
    }
    // blocking タスクは呼び出し元のスパンを引き継がないので、明示的に渡す。
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        span.in_scope(|| {
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
                return resolve_chunk(&leaves);
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
                for (value, flex_ids) in partial {
                    merge_into(&mut out, &value, flex_ids);
                }
            }
            Ok(out)
        })
    })
    .await
    .map_err(|e| AppError::InternalError(format!("leaf resolution task: {e}")))?
}

/// 初出のときだけバイト列を確保する。
fn merge_into(by_value: &mut ValueMap, value: &[u8], mut flex_ids: Vec<FlexId>) {
    match by_value.get_mut(value) {
        Some(existing) => existing.append(&mut flex_ids),
        None => {
            by_value.insert(value.to_vec(), flex_ids);
        }
    }
}

/// 範囲に重なる `(FlexId, 値)` を葉から取り出し、その場で `V` へ復元する。
///
/// `Source::read_range_ids` の内側（＝クエリ実行器を回している blocking タスクの上）から呼ばれる
/// ので、さらに blocking タスクへ出さずその場で並列化する。復元できない値はここで落とす。
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
