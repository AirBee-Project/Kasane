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

/// `data_get_stream` を Rayon で並列化する基準。
const DATA_GET_STREAM_PARALLEL_THRESHOLD: usize = 1000;

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
    ) -> Result<ValueGroups, AppError> {
        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();

        // ルーティングは 1 回だけ（並列チャンクごとの再ルーティングをやめる）。
        let by_leaf = shard::route_leaves_batched(
            &self.db.tables_data,
            &self.read_txn,
            table_id,
            flex_ids.iter(),
        )?;
        let parallel = by_leaf.len() >= DATA_GET_LEAF_PARALLEL_THRESHOLD;

        let by_value: ValueMap = if !parallel {
            let mut by_value = ValueMap::default();
            for (region, queries) in by_leaf {
                Self::resolve_leaf(
                    &self.db.tables_data,
                    &self.read_txn,
                    table_id,
                    &region,
                    &queries,
                    &mut by_value,
                )?;
            }
            by_value
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
                        Self::resolve_leaf(
                            &tables_data,
                            &txn,
                            table_id,
                            region,
                            queries,
                            &mut local,
                        )?;
                    }
                    Ok(local)
                })
                .collect::<Result<Vec<_>, _>>()?;

            // 部分マップを値でマージ。
            let mut by_value = ValueMap::default();
            for partial in partials {
                for (value, mut flex_ids) in partial {
                    by_value.entry(value).or_default().append(&mut flex_ids);
                }
            }
            by_value
        };

        Ok(by_value.into_iter().collect())
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
    ) -> Result<(), AppError> {
        let Some(arch) = shard::load_leaf_archived(tables_data, txn, table_id, region)? else {
            return Ok(());
        };

        // 葉ローカルの辞書 index で集約（中間 Vec を作らず整数キーでハッシュ）。
        let mut local: FxHashMap<u32, Vec<FlexId>> = FxHashMap::default();
        for query_flex in queries {
            arch.get_indexed(query_flex, |got_flex, packed| {
                local.entry(packed).or_default().push(got_flex);
            });
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

pub type DataStreamSender = tokio::sync::mpsc::Sender<Result<(Vec<u8>, Vec<FlexId>), AppError>>;

impl<'a> KasaneDbRead<'a> {
    #[tracing::instrument(skip_all)]
    pub fn data_get_stream(
        &self,
        table_id: TableId,
        flex_ids: SpatialIdSet,
        sender: DataStreamSender,
    ) {
        let tables_data = self.db.tables_data;
        let env = self.db.env.clone();

        tokio::task::spawn_blocking(move || {
            let flex_ids_vec: Vec<FlexId> = flex_ids.flex_ids().collect();
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

    /// `queries` 内の各クエリ FlexId をルーティングして解決し、`by_value` へ蓄積する。
    /// （`data_get_stream` 用。`data_get` はルーティングを 1 回だけ行い葉単位で並列化する。）
    fn resolve_query_batch(
        tables_data: &heed::Database<crate::db_init::TableIdAndFlexId, heed::types::Bytes>,
        txn: &heed::RoTxn<heed::WithoutTls>,
        table_id: TableId,
        queries: &[FlexId],
        by_value: &mut FxHashMap<Vec<u8>, Vec<FlexId>>,
    ) -> Result<(), AppError> {
        let by_leaf = shard::route_leaves_batched(tables_data, txn, table_id, queries.iter())?;
        for (region, queries) in by_leaf {
            Self::resolve_leaf(tables_data, txn, table_id, &region, &queries, by_value)?;
        }
        Ok(())
    }

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
