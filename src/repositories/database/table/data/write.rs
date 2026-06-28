use kasane_logic::{SpatialId, SpatialIdSet};

use crate::{error::AppError, repositories::KasaneDbWrite};

impl<'a> KasaneDbWrite<'a> {
    /// リポジトリ層にデータを挿入するための関数
    /// 挿入される空間IDの競合検証は行われない状態で受け取る。（シャードの内部で検証したほうが高速であるため。また、どうせシャードの内部で衝突する可能性があるため）
    pub fn data_insert<I: SpatialId>(
        &mut self,
        table_id: crate::models::id::TableId,
        ids: impl Iterator<Item = I>,
        data: &[u8],
    ) -> Result<(), AppError> {
        for spatial_id in ids {
            for flex_id in spatial_id.into_flex_ids() {
                // シャード空間をたどっていく
            }
        }

        todo!()
    }

    pub fn data_upsert(
        &mut self,
        table_id: crate::models::id::TableId,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        todo!()
    }

    pub fn data_remove(
        &mut self,
        table_id: crate::models::id::TableId,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        todo!()
    }
}
