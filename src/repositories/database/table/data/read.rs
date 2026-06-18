use kasane_logic::{IterSingleIds, SingleId, SpatialIdSet};

use crate::{error::AppError, repositories::KasaneDbRead};

type SpatialDataResult<'a> = Result<Option<(SingleId, &'a [u8])>, AppError>;
type SpatialDataListResult<'a> = Result<Option<Vec<(SingleId, &'a [u8])>>, AppError>;

impl<'a> KasaneDbRead<'a> {
    pub fn data_get(
        &self,
        table_id: uuid::Uuid,
        ids: SpatialIdSet,
    ) -> Result<Vec<(SingleId, Vec<u8>)>, AppError> {
        let mut result = Vec::new();

        let spatial_db = self.db.spatialid_to_value;

        for single_id in ids.iter_single_ids() {
            if let Some(data) =
                Self::overlap_equal(&spatial_db, &self.read_txn, table_id, &single_id)?
            {
                result.push((single_id, data.to_vec()));
                continue;
            };

            if let Some((_, data)) =
                Self::overlap_parent(&spatial_db, &self.read_txn, table_id, &single_id)?
            {
                result.push((single_id, data.to_vec()));
                continue;
            }

            if let Some(children_single_ids) =
                Self::overlap_children(&spatial_db, &self.read_txn, table_id, &single_id)?
            {
                for (child_single_id, value) in children_single_ids {
                    result.push((child_single_id, value.to_vec()));
                }
            }
        }

        Ok(result)
    }

    pub(self) fn overlap_equal<'txn>(
        spatial_db: &heed::Database<crate::db_init::TableIdAndSpatialId, heed::types::Bytes>,
        txn: &'txn heed::RoTxn<'a, heed::WithTls>,
        table_id: uuid::Uuid,
        target: &SingleId,
    ) -> Result<Option<&'txn [u8]>, AppError> {
        if let Some(val) = spatial_db.get(txn, &(table_id.into_bytes(), target.spatial_encode()))? {
            return Ok(Some(val));
        }
        Ok(None)
    }

    pub(self) fn overlap_parent<'txn>(
        spatial_db: &heed::Database<crate::db_init::TableIdAndSpatialId, heed::types::Bytes>,
        txn: &'txn heed::RoTxn<'a, heed::WithTls>,
        table_id: uuid::Uuid,
        target: &SingleId,
    ) -> SpatialDataResult<'txn> {
        for parent in target.spatial_parents() {
            if let Some(val) =
                spatial_db.get(txn, &(table_id.into_bytes(), parent.spatial_encode()))?
            {
                return Ok(Some((parent, val)));
            }
        }
        Ok(None)
    }

    pub(self) fn overlap_children<'txn>(
        spatial_db: &heed::Database<crate::db_init::TableIdAndSpatialId, heed::types::Bytes>,
        txn: &'txn heed::RoTxn<'a, heed::WithTls>,
        table_id: uuid::Uuid,
        target: &SingleId,
    ) -> SpatialDataListResult<'txn> {
        let mut result = Vec::new();

        let start = (table_id.into_bytes(), target.spatial_encode());
        let end = (table_id.into_bytes(), target.spatial_encode_prefix_max());

        for item in spatial_db.range(txn, &(start..=end))? {
            let (key, value) = item?;
            let single_id_encode = key.1;

            if SingleId::spatial_decode(&single_id_encode)? == *target {
                continue;
            }

            result.push((SingleId::spatial_decode(&single_id_encode)?, value));
        }

        if result.is_empty() {
            return Ok(None);
        }

        Ok(Some(result))
    }
}
