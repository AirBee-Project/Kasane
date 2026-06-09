use kasane_logic::{IterSingleIds, SingleId, SpatialIdSet};
use redb::{AccessGuard, ReadOnlyTable};

use crate::{db_init::SPATIALID_TO_VALUE, error::AppError, repositories::KasaneDbRead};

type SpatialDataResult<'a> = Result<Option<(SingleId, AccessGuard<'a, &'static [u8]>)>, AppError>;
type SpatialDataListResult<'a> =
    Result<Option<Vec<(SingleId, AccessGuard<'a, &'static [u8]>)>>, AppError>;

impl KasaneDbRead {
    ///データを取得する
    pub fn data_get(
        &self,
        db_name: &str,
        table_name: &str,
        ids: SpatialIdSet,
    ) -> Result<Vec<(SingleId, Vec<u8>)>, AppError> {
        //存在検証
        let table_meta = match self.table_info(db_name, table_name)? {
            Some(v) => v,
            None => {
                return Err(AppError::TableNotFound {
                    name: table_name.to_string(),
                });
            }
        };

        let mut result = Vec::new();

        let redb_spatial_ids = self.read_txn.open_table(SPATIALID_TO_VALUE)?;

        for single_id in ids.iter_single_ids() {
            //single_idと完全に等しい位置を調べる
            if let Some(access_guard) =
                Self::overlap_equal(&redb_spatial_ids, table_meta.id, &single_id)?
            {
                result.push((single_id, access_guard.value().to_vec()));
                continue;
            };

            //single_idの親を調べる
            if let Some((_, access_guard)) =
                Self::overlap_parent(&redb_spatial_ids, table_meta.id, &single_id)?
            {
                result.push((single_id, access_guard.value().to_vec()));
                continue;
            }

            //single_idの子を調べる
            if let Some(children_single_ids) =
                Self::overlap_children(&redb_spatial_ids, table_meta.id, &single_id)?
            {
                for (child_single_id, value) in children_single_ids {
                    result.push((child_single_id, value.value().to_vec()));
                }
            }
        }

        Ok(result)
    }

    ///入力された[SingleId]と同じかつ、[SingleId]が存在するかを検証する
    ///
    /// 存在した場合には値の参照を返す
    pub(self) fn overlap_equal<'a>(
        redb_spatial_ids: &ReadOnlyTable<([u8; 16], [u8; 12]), &'static [u8]>,
        table_id: uuid::Uuid,
        target: &SingleId,
    ) -> Result<Option<AccessGuard<'a, &'static [u8]>>, AppError> {
        if let Some(access_guard) =
            redb_spatial_ids.get((table_id.into_bytes(), target.spatial_encode()))?
        {
            return Ok(Some(access_guard));
        }
        Ok(None)
    }

    ///入力された[SingleId]の親となる[SingleId]が存在するかを確かめる
    ///
    /// 存在した場合には[SingleId]と値の参照を返す
    pub(self) fn overlap_parent<'a>(
        redb_spatial_ids: &ReadOnlyTable<([u8; 16], [u8; 12]), &'static [u8]>,
        table_id: uuid::Uuid,
        target: &SingleId,
    ) -> SpatialDataResult<'a> {
        for parent in target.spatial_parents() {
            if let Some(access_guard) =
                redb_spatial_ids.get((table_id.into_bytes(), parent.spatial_encode()))?
            {
                return Ok(Some((parent, access_guard)));
            }
        }
        Ok(None)
    }

    /// 入力した[SingleId]に含まれる[SingleId]が存在するかを確かめる
    ///
    /// 存在した場合には[SingleId]と値の参照を返す
    pub(self) fn overlap_children<'a>(
        redb_spatial_ids: &ReadOnlyTable<([u8; 16], [u8; 12]), &'static [u8]>,
        table_id: uuid::Uuid,
        target: &SingleId,
    ) -> SpatialDataListResult<'a> {
        let mut result: Vec<_> = Vec::new();

        for ele in redb_spatial_ids.range(
            (table_id.into_bytes(), target.spatial_encode())
                ..=(table_id.into_bytes(), target.spatial_encode_prefix_max()),
        )? {
            let (key, value) = ele?;
            let (_, single_id_encode) = key.value();

            //equalの排除
            if SingleId::spatial_decode(&single_id_encode)? == *target {
                continue;
            }

            //resultに対する挿入
            result.push((SingleId::spatial_decode(&single_id_encode)?, value));
        }

        if result.is_empty() {
            return Ok(None);
        }

        Ok(Some(result))
    }
}
