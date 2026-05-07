use kasane_logic::{SingleId, SpatialIdSet};
use redb::ReadTransaction;

use crate::error::AppError;

pub struct SpatialDbRead {
    read_txn: ReadTransaction,
}

impl SpatialDbRead {
    pub fn new(read_txn: ReadTransaction) -> Self {
        Self { read_txn }
    }

    /// TODO: 実際のDB操作を実装する
    pub fn data_get(
        &self,
        _layer_id: u64,
        _ids: SpatialIdSet,
    ) -> Result<Vec<(SingleId, &[u8])>, AppError> {
        Ok(vec![])
    }
}
