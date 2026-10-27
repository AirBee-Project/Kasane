use std::sync::Arc;

use spatial_id::RangeId;
use spatial_id::collection::SpatialIdMap;
use spatial_id::query::{CancellationToken, QueryIter, SafeValue, Source};

use super::{Memory, State};
use crate::models::id::TableId;
use crate::repositories::traits::DecodeFn;

pub struct MemSource<V> {
    map: SpatialIdMap<Vec<u8>>,
    decode: DecodeFn<V>,
}

impl Memory {
    pub fn table_source<V>(
        &self,
        table_id: TableId,
        decode: DecodeFn<V>,
        snapshot: Arc<State>,
    ) -> MemSource<V> {
        MemSource {
            map: snapshot.data.get(&table_id).cloned().unwrap_or_default(),
            decode,
        }
    }
}

impl<V: SafeValue> Source for MemSource<V> {
    type Value = V;

    fn read_flex_ids<'a>(
        &'a self,
        targets: &[RangeId],
        token: &CancellationToken,
    ) -> Result<QueryIter<'a, V>, spatial_id::Error> {
        let stored = self.map.read_flex_ids(targets, token)?;
        // 復元できない値（型に合わない格納値）の FlexId は結果に含めない。
        Ok(QueryIter::from_ordered(stored.filter_map(
            move |item| match item {
                Ok((id, bytes)) => (self.decode)(&bytes).map(|value| Ok((id, value))),
                Err(e) => Some(Err(e)),
            },
        )))
    }
}
