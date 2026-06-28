use kasane_logic::{FlexId, IterFlexIds, SpatialIdSet};

use super::shard;
use crate::{error::AppError, repositories::KasaneDbRead};

impl<'a> KasaneDbRead<'a> {
    /// 空間IDごとに格納値を解決して返す。
    ///
    /// `decode` は「ストレージ上のバイト列 → 任意の復元値」への変換関数。
    /// 返すのは `(FlexId, T)` で、[`SingleId`](kasane_logic::SingleId) への展開は上位レイヤーが行う。
    pub fn data_get<T, F>(
        &self,
        table_id: crate::models::id::TableId,
        ids: SpatialIdSet,
        decode: F,
    ) -> Result<Vec<(FlexId, T)>, AppError>
    where
        F: Fn(&[u8]) -> Result<T, AppError> + Sync,
        T: Send,
    {
        let mut result = Vec::new();
        for query_flex in ids.iter_flex_ids() {
            // ポインタツリーを辿って query_flex と重なるリーフをすべて取得。
            for region in
                shard::route_leaves(&self.db.tables_data, &self.read_txn, table_id, &query_flex)?
            {
                let map =
                    shard::load_leaf_map(&self.db.tables_data, &self.read_txn, table_id, &region)?;
                // query_flex に切り取られた (FlexId, 値) を復元して返す。
                for (got_flex, value) in map.get(&query_flex) {
                    result.push((got_flex, decode(value)?));
                }
            }
        }
        Ok(result)
    }
}
