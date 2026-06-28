use kasane_logic::{IterSingleIds, SingleId, SpatialIdSet};
use rayon::prelude::*;

use crate::{error::AppError, repositories::KasaneDbRead};

type SpatialDataResult<'txn> = Result<Option<(SingleId, &'txn [u8])>, AppError>;
type SpatialDataListResult<'txn> = Result<Option<Vec<(SingleId, &'txn [u8])>>, AppError>;

const PARALLEL_FANOUT_THRESHOLD: usize = 512;

impl<'a> KasaneDbRead<'a> {
    pub fn data_get<T, F>(
        &self,
        table_id: crate::models::id::TableId,
        ids: SpatialIdSet,
        decode: F,
    ) -> Result<Vec<(SingleId, T)>, AppError>
    where
        F: Fn(&[u8]) -> Result<T, AppError> + Sync,
        T: Send,
    {
        todo!()
    }
}
