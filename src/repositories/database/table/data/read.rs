use std::collections::HashMap;

use kasane_logic::{FlexId, IterFlexIds, SpatialIdSet};

use super::{shard, value_index};
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::{error::AppError, repositories::KasaneDbRead};

/// `data_get` の戻り値：`(値バイト, その値を持つ FlexId 群)` の一覧。
type ValueGroups = Vec<(Vec<u8>, Vec<FlexId>)>;

/// クエリ FlexId 数がこれ以上なら `data_get` を Rayon で並列化する。
/// 各クエリは独立（route→load→get）なので、ワーカごとに read txn を開いて分担できる。
const DATA_GET_PARALLEL_THRESHOLD: usize = 64;

impl<'a> KasaneDbRead<'a> {
    /// 空間IDごとに格納値（生バイト列）を解決し、**値でグループ化**して返す。
    ///
    /// 1つの値が複数の [`FlexId`] に割り当たるケース（同値の離れた複数領域や、
    /// コアースなリーフのクエリ切り取り）があるため、`(値バイト, その値を持つ FlexId 群)`
    /// の形で返す。これにより値バイトの重複保持と、上位での重複復元を避けられる。
    /// 値の解釈（[`restore_value`] 等）と [`SingleId`](kasane_logic::SingleId) への展開は上位レイヤーが行う。
    ///
    /// クエリ数が多いときは Rayon でクエリを分割し、ワーカごとに read txn を開いて
    /// 部分結果を並列に集め、最後に値でマージする。
    pub fn data_get(
        &self,
        table_id: crate::models::id::TableId,
        ids: SpatialIdSet,
    ) -> Result<ValueGroups, AppError> {
        let flex_ids: Vec<FlexId> = ids.iter_flex_ids().collect();

        if flex_ids.len() < DATA_GET_PARALLEL_THRESHOLD {
            let mut by_value: HashMap<Vec<u8>, Vec<FlexId>> = HashMap::new();
            Self::resolve_chunk(
                &self.db.tables_data,
                &self.read_txn,
                table_id,
                &flex_ids,
                &mut by_value,
            )?;
            return Ok(by_value.into_iter().collect());
        }

        // 並列パス：read txn は跨ぎ共有できないため、ワーカごとに env から開く。
        // 非 Sync な self.read_txn には触れず、Copy な Database と Clone な Env だけを渡す。
        use rayon::prelude::*;
        let tables_data = self.db.tables_data;
        let env = &self.db.env;

        let chunk_size = flex_ids.len().div_ceil(rayon::current_num_threads().max(1));
        let partials: Vec<HashMap<Vec<u8>, Vec<FlexId>>> = flex_ids
            .par_chunks(chunk_size.max(1))
            .map(|chunk| -> Result<HashMap<Vec<u8>, Vec<FlexId>>, AppError> {
                let txn = env
                    .read_txn()
                    .map_err(|e| AppError::InternalError(e.to_string()))?;
                let mut local: HashMap<Vec<u8>, Vec<FlexId>> = HashMap::new();
                Self::resolve_chunk(&tables_data, &txn, table_id, chunk, &mut local)?;
                Ok(local)
            })
            .collect::<Result<Vec<_>, _>>()?;

        // 部分マップを値でマージ。
        let mut by_value: HashMap<Vec<u8>, Vec<FlexId>> = HashMap::new();
        for partial in partials {
            for (value, mut flex_ids) in partial {
                by_value.entry(value).or_default().append(&mut flex_ids);
            }
        }
        Ok(by_value.into_iter().collect())
    }

    /// `chunk` 内の各クエリ FlexId を解決し、`by_value`（値→FlexId群）へ蓄積する。
    /// `data_get` の直列パスと並列ワーカで共有するコア処理。
    fn resolve_chunk(
        tables_data: &heed::Database<crate::db_init::TableIdAndFlexId, heed::types::Bytes>,
        txn: &heed::RoTxn<heed::WithoutTls>,
        table_id: TableId,
        chunk: &[FlexId],
        by_value: &mut HashMap<Vec<u8>, Vec<FlexId>>,
    ) -> Result<(), AppError> {
        for query_flex in chunk {
            // ポインタツリーを辿って query_flex と重なるリーフをすべて取得。
            for region in shard::route_leaves(tables_data, txn, table_id, query_flex)? {
                // ZeroCopy archived リーダで、Arc 木を再構築せずに走査する。
                let Some(arch) = shard::load_leaf_archived(tables_data, txn, table_id, &region)?
                else {
                    continue; // 未作成リーフ（データなし）
                };
                // query_flex に切り取られた (FlexId, 値) を、値ごとにまとめる。
                for (got_flex, value) in arch.get(query_flex) {
                    // 値バイトのクローンは初出時のみ（既出値は get_mut で参照のみ）。
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
