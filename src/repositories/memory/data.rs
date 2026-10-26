use std::collections::HashMap;
use std::sync::Arc;

use spatial_id::FlexId;
use spatial_id::collection::{SpatialIdMap, SpatialIdSet};

use super::State;
use crate::models::id::TableId;
use crate::repositories::ValueGroups;

impl State {
    pub(super) fn table_count(&self, table_id: TableId) -> u64 {
        self.data.get(&table_id).map_or(0, |map| map.count() as u64)
    }

    fn map_mut(&mut self, table_id: TableId) -> &mut SpatialIdMap<Vec<u8>> {
        Arc::make_mut(&mut self.data).entry(table_id).or_default()
    }

    pub(super) fn data_get(&self, table_id: TableId, ids: SpatialIdSet) -> ValueGroups {
        let Some(map) = self.data.get(&table_id) else {
            return Vec::new();
        };
        let mut by_value: HashMap<&[u8], Vec<FlexId>> = HashMap::new();
        for id in ids.iter() {
            for (found, value) in map.get(id) {
                by_value.entry(value.as_slice()).or_default().push(found);
            }
        }
        by_value
            .into_iter()
            .map(|(value, ids)| (value.to_vec(), ids))
            .collect()
    }

    pub(super) fn data_insert(&mut self, table_id: TableId, ids: SpatialIdSet, data: &[u8]) {
        let map = self.map_mut(table_id);
        for id in ids.iter() {
            map.insert(id, data.to_vec());
        }
    }

    pub(super) fn data_upsert(&mut self, table_id: TableId, ids: SpatialIdSet, data: &[u8]) {
        let map = self.map_mut(table_id);
        for id in ids.iter() {
            map.upsert(id, data.to_vec());
        }
    }

    pub(super) fn data_remove(&mut self, table_id: TableId, ids: SpatialIdSet) {
        if let Some(map) = Arc::make_mut(&mut self.data).get_mut(&table_id) {
            for id in ids.iter() {
                map.remove(id);
            }
        }
    }
}
