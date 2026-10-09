//! テーブルの中身（空間 ID と値の対応）の読み書き。

use kasane_logic::{FlexId, SpatialIdMap, SpatialIdSet};
use rustc_hash::FxHashMap;

use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::repositories::ValueGroups;
use crate::repositories::encoding::value_index::order_preserving;

use super::State;

impl State {
    /// `ids` と重なる部分の値を、値ごとにまとめて返す。
    pub(super) fn data_get_impl(&self, table_id: TableId, ids: SpatialIdSet) -> ValueGroups {
        let Some(map) = self.data.get(&table_id) else {
            return Vec::new();
        };
        let mut by_value: FxHashMap<Vec<u8>, Vec<FlexId>> = FxHashMap::default();
        for target in ids.flex_ids() {
            for (flex_id, value) in map.get(&target) {
                // 値の複製は distinct な値ごとに 1 回だけにする。
                match by_value.get_mut(value) {
                    Some(flex_ids) => flex_ids.push(flex_id),
                    None => {
                        by_value.insert(value.clone(), vec![flex_id]);
                    }
                }
            }
        }
        by_value.into_iter().collect()
    }

    pub(super) fn data_filter_eq_impl(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        value: &[u8],
    ) -> Vec<FlexId> {
        let wanted = order_preserving(data_type, value);
        self.filter_values(table_id, data_type, |vkey| vkey == wanted)
    }

    /// `lo`〜`hi` は両端を含む。
    pub(super) fn data_filter_range_impl(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        lo: &[u8],
        hi: &[u8],
    ) -> Vec<FlexId> {
        let lo = order_preserving(data_type, lo);
        let hi = order_preserving(data_type, hi);
        self.filter_values(table_id, data_type, |vkey| {
            lo.as_slice() <= vkey && vkey <= hi.as_slice()
        })
    }

    pub(super) fn data_insert_impl(&mut self, table_id: TableId, ids: SpatialIdSet, data: &[u8]) {
        let map = self.data_mut(table_id);
        for flex_id in ids.flex_ids() {
            map.insert(flex_id, data.to_vec());
        }
    }

    /// まだ値が無い空間 ID にだけ書く（既存値は保持）。
    pub(super) fn data_upsert_impl(&mut self, table_id: TableId, ids: SpatialIdSet, data: &[u8]) {
        let map = self.data_mut(table_id);
        for flex_id in ids.flex_ids() {
            map.upsert(flex_id, data.to_vec());
        }
    }

    pub(super) fn data_remove_impl(&mut self, table_id: TableId, ids: SpatialIdSet) {
        let map = self.data_mut(table_id);
        for flex_id in ids.flex_ids() {
            map.remove(&flex_id);
        }
    }

    fn data_mut(&mut self, table_id: TableId) -> &mut SpatialIdMap<Vec<u8>> {
        self.data.entry(table_id).or_default()
    }

    /// 値インデックスを持たないので全件を走査する。比較は索引と同じ順序保存エンコードで行う。
    fn filter_values(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        keep: impl Fn(&[u8]) -> bool,
    ) -> Vec<FlexId> {
        self.data.get(&table_id).map_or_else(Vec::new, |map| {
            map.iter()
                .filter(|(_, value)| keep(&order_preserving(data_type, value)))
                .map(|(flex_id, _)| flex_id)
                .collect()
        })
    }
}
