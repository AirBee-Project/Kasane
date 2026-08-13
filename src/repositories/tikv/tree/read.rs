//! 読み取りの入口。`TikvRead` に生やす固有メソッドはここだけ。
//!
//! 降下（ネットワーク）は [`routing`](super::routing) に任せ、ここは
//! **降下で決まった葉から値を取り出す**側を持つ。葉の走査は FlexId 数に比例する
//! CPU 処理なので、非同期ワーカーを占有しないよう blocking タスクへ出したうえで、
//! 葉が多ければ rayon で分散する（モジュール冒頭の「CPU をどこで回すか」）。

use std::sync::atomic::{AtomicUsize, Ordering};

use kasane_logic::{FlexId, RangeId, SpatialIdSet};
use rustc_hash::FxHashMap;

use super::node::archived_leaf;
use super::routing::{RoutedLeaf, RoutedRange, route_leaves_batched, route_leaves_for_ranges};
use super::{
    AppError, LEAF_PARALLEL_THRESHOLD, Reader, TableDataType, TableId, TikvRead, ValueMap, keys, kv,
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
    pub(in crate::repositories::tikv) async fn read_values_in_ranges<V: Send>(
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
    for (packed, flex_ids) in local {
        merge_into(by_value, arch.value_bytes(packed), flex_ids);
    }
    Ok(())
}

/// 葉をまとめて解決する。
///
/// 葉は互いに独立（同一 FlexId は 1 つの葉にしか属さない）なので、部分マップを作って
/// 最後に値でマージするだけで正しく合流できる。走査は blocking タスクの上で行い、
/// 葉が多ければさらに rayon で分散する。
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

/// 同じ値へ集まった FlexId を 1 つのマップへ寄せる。
/// 既にある値には追記し、初出のときだけバイト列を確保する。
fn merge_into(by_value: &mut ValueMap, value: &[u8], mut flex_ids: Vec<FlexId>) {
    match by_value.get_mut(value) {
        Some(existing) => existing.append(&mut flex_ids),
        None => {
            by_value.insert(value.to_vec(), flex_ids);
        }
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
