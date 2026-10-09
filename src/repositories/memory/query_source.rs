//! テーブルの [`SpatialIdMap`] を Kasane-Logic のクエリ入力源として見せるアダプタ。

use std::sync::Arc;

use kasane_logic::{
    CancellationToken, Error as LogicError, FlexId, RangeId, SafeValue, Source, SpatialIdMap,
    WorkingTree,
};

use crate::models::id::TableId;
use crate::repositories::traits::DecodeFn;

use super::{MemoryDb, State};

/// 1 テーブル分のクエリの入力源。断面を取った時点の [`SpatialIdMap`] を持つ。
pub struct TableSource<V> {
    map: SpatialIdMap<Vec<u8>>,
    decode: DecodeFn<V>,
}

impl MemoryDb {
    /// `snapshot` 時点の `table_id` のテーブルを読む入力源を作る。
    pub fn table_source<V>(
        &self,
        table_id: TableId,
        decode: DecodeFn<V>,
        snapshot: Arc<State>,
    ) -> TableSource<V> {
        TableSource {
            map: snapshot.data.get(&table_id).cloned().unwrap_or_default(),
            decode,
        }
    }
}

impl<V> TableSource<V> {
    fn decoded<'a>(
        &'a self,
        entries: impl Iterator<Item = (FlexId, &'a Vec<u8>)> + 'a,
    ) -> impl Iterator<Item = (FlexId, V)> + 'a {
        // 復元できない値（型に合わない格納値）の FlexId は結果に含めない。
        entries.filter_map(|(flex_id, raw)| (self.decode)(raw).map(|value| (flex_id, value)))
    }
}

impl<V> Source for TableSource<V>
where
    V: SafeValue + 'static,
{
    type Value = V;

    fn read_range_ids(
        &self,
        bounds: &[RangeId],
        token: &CancellationToken,
    ) -> Result<WorkingTree<V>, LogicError> {
        let mut out = Vec::new();
        for range in bounds {
            if token.is_cancelled() {
                return Err(LogicError::Cancelled);
            }
            out.extend(self.decoded(self.map.get_overlapping(range)));
        }
        // 重なり合う bounds が同じ FlexId を複数回返しても、値は同じなので union が吸収する。
        Ok(out.into_iter().collect())
    }

    fn read_all(self: Box<Self>, _token: &CancellationToken) -> Result<WorkingTree<V>, LogicError> {
        Ok(self.decoded(self.map.iter()).collect())
    }
}
