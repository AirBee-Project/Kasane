use kasane_logic::{SingleId, SpatialIdSet};

use crate::{
    error::AppError,
    repositories::layer::{read::SpatialDbRead, write::SpatialDbWrite},
};

impl SpatialDbRead {
    ///データを取得する
    pub fn data_get(
        &self,
        layer_name: &str,
        ids: SpatialIdSet,
    ) -> Result<Vec<(SingleId, Vec<u8>)>, AppError> {
        todo!()
    }
}

impl SpatialDbWrite {}
