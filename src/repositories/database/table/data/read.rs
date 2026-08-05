use rustc_hash::FxHashMap;

use kasane_logic::{FlexId, SpatialIdSet};

use super::{shard, value_index};
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::{error::AppError, repositories::KasaneDbRead};

/// `data_get` の戻り値：`(値バイト, その値を持つ FlexId 群)` の一覧。
type ValueGroups = Vec<(Vec<u8>, Vec<FlexId>)>;

type ValueMap = FxHashMap<Vec<u8>, Vec<FlexId>>;

/// `data_get` を葉単位で Rayon 並列化する基準（触れる**リーフ数**）。
///
/// 並列化の基準は「クエリ FlexId 数」ではなく「実際に触れる葉の数」に置く。
/// 広域検索はクエリ FlexId が少数（数個）でも数千の葉・数百万セルに及ぶため、
/// クエリ数で判定すると単一スレッドに落ちてしまう（実測: 540万セルで 363ms）。
const DATA_GET_LEAF_PARALLEL_THRESHOLD: usize = 32;

impl<'a> KasaneDbRead<'a> {
    /// 指定された範囲の空間IDを値ごとにグループ化して返す。
    ///
    /// ルーティング（ポインタ木の降下）は 1 回だけ行い、担当リーフへ振り分けたあと、
    /// **リーフ単位**で解決を並列化する。各リーフは互いに独立（同一セルは 1 つの葉にしか
    /// 属さない）なので、部分マップを作って最後に値でマージするだけで正しく合流できる。
    #[tracing::instrument(skip_all)]
    pub fn data_get(
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
    /// `by_value`（値→FlexId群）へ蓄積する。
    ///
    /// セルごとに値バイト列でハッシュすると、値の種類が少数でも巨大な結果（数百万セル）で
    /// 同じバイト列を何百万回もハッシュすることになる。そこでまず**この葉ローカルの辞書
    /// インデックス（u32）**でグルーピングし（整数ハッシュは軽い）、葉に現れた distinct 値の
    /// 数だけ実バイト列へ復元して全体マップへマージする。
    fn resolve_leaf(
        tables_data: &heed::Database<crate::db_init::TableIdAndFlexId, heed::types::Bytes>,
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
    #[tracing::instrument(skip_all)]
    pub fn data_filter_eq(
        &'a self,
        table_id: TableId,
        data_type: TableDataType,
        value: &[u8],
    ) -> Result<impl Iterator<Item = Result<FlexId, AppError>> + 'a, AppError> {
        let prefix =
            value_index::make_prefix(table_id, &value_index::order_preserving(data_type, value));

        let iter = self
            .db
            .value_index
            .prefix_iter(&self.read_txn, prefix.as_slice())?;

        Ok(iter.filter_map(move |item| match item {
            Ok((key, _)) => {
                // 可変長値で前方一致しただけの別キーを除外（残りがちょうど flexid 分の長さ）。
                if key.len() != prefix.len() + FlexId::ENCODED_LEN {
                    return None;
                }
                Some(value_index::flexid_from_key(key))
            }
            Err(e) => Some(Err(AppError::InternalError(e.to_string()))),
        }))
    }

    #[tracing::instrument(skip_all)]
    pub fn data_filter_range(
        &'a self,
        table_id: TableId,
        data_type: TableDataType,
        lo: &[u8],
        hi: &[u8],
    ) -> Result<impl Iterator<Item = Result<FlexId, AppError>> + 'a, AppError> {
        let start =
            value_index::make_prefix(table_id, &value_index::order_preserving(data_type, lo));
        // hi 側は flexid 部を最大化して `(hi, *)` まで含める。
        let mut end =
            value_index::make_prefix(table_id, &value_index::order_preserving(data_type, hi));
        end.extend_from_slice(&[0xFF; FlexId::ENCODED_LEN]);

        let bounds = (
            std::ops::Bound::Included(start.as_slice()),
            std::ops::Bound::Included(end.as_slice()),
        );
        let iter = self.db.value_index.range(&self.read_txn, &bounds)?;

        Ok(iter.map(|item| match item {
            Ok((key, _)) => value_index::flexid_from_key(key),
            Err(e) => Err(AppError::InternalError(e.to_string())),
        }))
    }
}
