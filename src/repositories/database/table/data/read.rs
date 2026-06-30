use rustc_hash::FxHashMap;

use kasane_logic::{FlexId, IterFlexIds, SpatialIdSet};

use super::{shard, value_index};
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::{error::AppError, repositories::KasaneDbRead};

/// `data_get` の戻り値：`(値バイト, その値を持つ FlexId 群)` の一覧。
type ValueGroups = Vec<(Vec<u8>, Vec<FlexId>)>;

type ValueMap = FxHashMap<Vec<u8>, Vec<FlexId>>;

/// `data_get` を 並列化する基準。
const DATA_GET_PARALLEL_THRESHOLD: usize = 500;

/// `data_get_stream` を Rayon で並列化する基準。
const DATA_GET_STREAM_PARALLEL_THRESHOLD: usize = 1000;

impl<'a> KasaneDbRead<'a> {
    /// 指定された範囲の空間IDを値ごとにグループ化して返す。
    pub fn data_get(
        &self,
        table_id: crate::models::id::TableId,
        ids: SpatialIdSet,
    ) -> Result<ValueGroups, AppError> {
        let flex_ids: Vec<FlexId> = ids.iter_flex_ids().collect();

        if flex_ids.len() < DATA_GET_PARALLEL_THRESHOLD {
            let mut by_value: FxHashMap<Vec<u8>, Vec<FlexId>> = FxHashMap::default();
            Self::resolve_query_batch(
                &self.db.tables_data,
                &self.read_txn,
                table_id,
                &flex_ids,
                &mut by_value,
            )?;
            return Ok(by_value.into_iter().collect());
        }

        use rayon::prelude::*;
        let tables_data = self.db.tables_data;
        let env = &self.db.env;

        let chunk_size = flex_ids.len().div_ceil(rayon::current_num_threads().max(1));
        let partials: Vec<FxHashMap<Vec<u8>, Vec<FlexId>>> = flex_ids
            .par_chunks(chunk_size.max(1))
            .map(
                |chunk| -> Result<FxHashMap<Vec<u8>, Vec<FlexId>>, AppError> {
                    let txn = env
                        .read_txn()
                        .map_err(|e| AppError::InternalError(e.to_string()))?;
                    let mut local: FxHashMap<Vec<u8>, Vec<FlexId>> = FxHashMap::default();
                    Self::resolve_query_batch(&tables_data, &txn, table_id, chunk, &mut local)?;
                    Ok(local)
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        // 部分マップを値でマージ。
        let mut by_value: FxHashMap<Vec<u8>, Vec<FlexId>> = FxHashMap::default();
        for partial in partials {
            for (value, mut flex_ids) in partial {
                by_value.entry(value).or_default().append(&mut flex_ids);
            }
        }
        Ok(by_value.into_iter().collect())
    }
}

pub type DataStreamSender = tokio::sync::mpsc::Sender<Result<(Vec<u8>, Vec<FlexId>), AppError>>;

impl<'a> KasaneDbRead<'a> {
    pub fn data_get_stream(
        &self,
        table_id: TableId,
        flex_ids: SpatialIdSet,
        sender: DataStreamSender,
    ) {
        let tables_data = self.db.tables_data;
        let env = self.db.env.clone();

        tokio::task::spawn_blocking(move || {
            let flex_ids_vec: Vec<FlexId> = flex_ids.iter_flex_ids().collect();
            if flex_ids_vec.is_empty() {
                return;
            }

            // 小規模: 単一スレッドで解決し逐次送信。
            if flex_ids_vec.len() < DATA_GET_STREAM_PARALLEL_THRESHOLD {
                let txn = match env.read_txn() {
                    Ok(txn) => txn,
                    Err(e) => {
                        let _ = sender.blocking_send(Err(AppError::InternalError(e.to_string())));
                        return;
                    }
                };
                let mut local = FxHashMap::default();
                if let Err(e) = Self::resolve_query_batch(
                    &tables_data,
                    &txn,
                    table_id,
                    &flex_ids_vec,
                    &mut local,
                ) {
                    let _ = sender.blocking_send(Err(e));
                    return;
                }
                for (val, ids) in local {
                    if sender.blocking_send(Ok((val, ids))).is_err() {
                        return; // receiver dropped (limit reached or stream closed)
                    }
                }
                return;
            }

            use rayon::prelude::*;
            let chunk_size = flex_ids_vec
                .len()
                .div_ceil(rayon::current_num_threads().max(1));

            let partials: Result<Vec<ValueMap>, AppError> = flex_ids_vec
                .par_chunks(chunk_size.max(1))
                .map(|chunk| {
                    let txn = env
                        .read_txn()
                        .map_err(|e| AppError::InternalError(e.to_string()))?;
                    let mut local = ValueMap::default();
                    Self::resolve_query_batch(&tables_data, &txn, table_id, chunk, &mut local)?;
                    Ok(local)
                })
                .collect();

            match partials {
                Ok(partials) => {
                    for partial in partials {
                        for (val, ids) in partial {
                            if sender.blocking_send(Ok((val, ids))).is_err() {
                                return; // receiver dropped
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = sender.blocking_send(Err(e));
                }
            }
        });
    }

    /// `queries` 内の各クエリ FlexId を解決し、`by_value`（値→FlexId群）へ蓄積する。
    fn resolve_query_batch(
        tables_data: &heed::Database<crate::db_init::TableIdAndFlexId, heed::types::Bytes>,
        txn: &heed::RoTxn<heed::WithoutTls>,
        table_id: TableId,
        queries: &[FlexId],
        by_value: &mut FxHashMap<Vec<u8>, Vec<FlexId>>,
    ) -> Result<(), AppError> {
        let by_leaf = shard::route_leaves_batched(tables_data, txn, table_id, queries)?;
        for (region, queries) in by_leaf {
            let Some(arch) = shard::load_leaf_archived(tables_data, txn, table_id, &region)? else {
                continue;
            };
            for query_flex in queries {
                for (got_flex, value) in arch.get(&query_flex) {
                    if let Some(flex_ids) = by_value.get_mut(value) {
                        flex_ids.push(got_flex);
                    } else {
                        by_value.insert(value.to_vec(), vec![got_flex]);
                    }
                }
            }
        }
        Ok(())
    }

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
                // 可変長値で前方一致しただけの別キーを除外（残りがちょうど flexid 14B）。
                if key.len() != prefix.len() + 14 {
                    return None;
                }
                Some(value_index::flexid_from_key(key))
            }
            Err(e) => Some(Err(AppError::InternalError(e.to_string()))),
        }))
    }

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
        end.extend_from_slice(&[0xFF; 14]);

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
