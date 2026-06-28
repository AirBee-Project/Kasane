use std::collections::HashMap;

use kasane_logic::{FlexId, IterFlexIds, SpatialIdSet};

use super::{shard, value_index};
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::{error::AppError, repositories::KasaneDbRead};

/// `data_get` の戻り値：`(値バイト, その値を持つ FlexId 群)` の一覧。
type ValueGroups = Vec<(Vec<u8>, Vec<FlexId>)>;

impl<'a> KasaneDbRead<'a> {
    /// 空間IDごとに格納値（生バイト列）を解決し、**値でグループ化**して返す。
    ///
    /// 1つの値が複数の [`FlexId`] に割り当たるケース（同値の離れた複数領域や、
    /// コアースなリーフのクエリ切り取り）があるため、`(値バイト, その値を持つ FlexId 群)`
    /// の形で返す。これにより値バイトの重複保持と、上位での重複復元を避けられる。
    /// 値の解釈（[`restore_value`] 等）と [`SingleId`](kasane_logic::SingleId) への展開は上位レイヤーが行う。
    pub fn data_get(
        &self,
        table_id: crate::models::id::TableId,
        ids: SpatialIdSet,
    ) -> Result<ValueGroups, AppError> {
        let mut by_value: HashMap<Vec<u8>, Vec<FlexId>> = HashMap::new();
        for query_flex in ids.iter_flex_ids() {
            // ポインタツリーを辿って query_flex と重なるリーフをすべて取得。
            for region in
                shard::route_leaves(&self.db.tables_data, &self.read_txn, table_id, &query_flex)?
            {
                // ZeroCopy archived リーダで、Arc 木を再構築せずに走査する。
                let Some(arch) = shard::load_leaf_archived(
                    &self.db.tables_data,
                    &self.read_txn,
                    table_id,
                    &region,
                )?
                else {
                    continue; // 未作成リーフ（データなし）
                };
                // query_flex に切り取られた (FlexId, 値) を、値ごとにまとめる。
                for (got_flex, value) in arch.get(&query_flex) {
                    // 値バイトのクローンは初出時のみ（既出値は get_mut で参照のみ）。
                    if let Some(flex_ids) = by_value.get_mut(value) {
                        flex_ids.push(got_flex);
                    } else {
                        by_value.insert(value.to_vec(), vec![got_flex]);
                    }
                }
            }
        }
        Ok(by_value.into_iter().collect())
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
