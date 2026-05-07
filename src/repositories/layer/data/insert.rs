use kasane_logic::{IterSingleIds, SingleId, SpatialIdSet};
use redb::{AccessGuard, ReadableTable, Table};

use crate::{db_init::SPATIAL_IDS, error::AppError, repositories::layer::write::SpatialDbWrite};

impl SpatialDbWrite {
    /// 空間IDに対して値を割り当てる
    pub fn data_insert(
        &self,
        layer_name: &str,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        //存在検証
        let layer_meta = match self.layer_info(layer_name)? {
            Some(v) => v,
            None => {
                return Err(AppError::LayerNotFound {
                    name: layer_name.to_string(),
                });
            }
        };

        let redb_spatial_ids = self.write_txn.open_table(SPATIAL_IDS)?;

        let mut should_remove: Vec<SingleId> = Vec::new();
        let mut should_insert: Vec<(SingleId, &[u8])> = Vec::new();

        for single_id in ids.iter_single_ids() {
            //single_idと完全に等しい位置を調べる
            if let Some(access_guard) =
                Self::overlap_equal(&redb_spatial_ids, layer_meta.id, &single_id)?
            {
                if access_guard.value() == data {
                    continue;
                } else {
                    //完全に等しいので上書きだけで良い
                    should_insert.push((single_id, data));
                    continue;
                }
            };

            //single_idの親を調べていく
            if let Some((parent_single_id, access_guard)) =
                Self::overlap_parent(&redb_spatial_ids, layer_meta.id, &single_id)?
            {
                if access_guard.value() == data {
                    continue;
                } else {
                    for fragment_single_id in parent_single_id.difference(&single_id) {
                        should_insert.push((fragment_single_id, access_guard.value()));
                        should_insert.push((single_id, data));
                        should_remove.push(parent_single_id);
                    }
                    continue;
                }
            }

            //single_idの子を調べていく
        }

        Ok(())
    }

    ///入力された[SingleId]と同じかつ、[SingleId]が存在するかを検証する
    ///
    /// 存在した場合には値の参照を返す
    fn overlap_equal<'a>(
        redb_spatial_ids: &'a Table<'_, (u64, [u8; 12]), &'static [u8]>,
        layer_id: u64,
        target: &SingleId,
    ) -> Result<Option<AccessGuard<'a, &'static [u8]>>, AppError> {
        if let Some(access_guard) = redb_spatial_ids.get((layer_id, target.spatial_encode()))? {
            return Ok(Some(access_guard));
        }
        Ok(None)
    }

    ///入力された[SingleId]の親となる[SingleId]が存在するかを確かめる
    ///
    /// 存在した場合には[SingleId]と値の参照を返す
    fn overlap_parent<'a>(
        redb_spatial_ids: &'a Table<'_, (u64, [u8; 12]), &'static [u8]>,
        layer_id: u64,
        target: &SingleId,
    ) -> Result<Option<(SingleId, AccessGuard<'a, &'static [u8]>)>, AppError> {
        for parent in target.spatial_parents() {
            if let Some(access_guard) = redb_spatial_ids.get((layer_id, parent.spatial_encode()))? {
                return Ok(Some((parent, access_guard)));
            }
        }
        Ok(None)
    }

    /// 入力した[SingleId]に含まれる[SingleId]が存在するかを確かめる
    ///
    /// 存在した場合には[SingleId]と値の参照を返す
    fn overlap_children(
        redb_spatial_ids: &Table<'_, (u64, [u8; 12]), &'static [u8]>,
        layer_id: u64,
        target: SingleId,
    ) -> Result<Option<Vec<SingleId>>, AppError> {
        let mut result: Vec<_> = Vec::new();

        for ele in redb_spatial_ids.range(
            (layer_id, target.spatial_encode())..=(layer_id, target.spatial_encode_prefix_max()),
        )? {
            let (key, _) = ele?;
            let (_, single_id_encode) = key.value();

            //equalの排除
            if SingleId::spatial_decode(&single_id_encode)? == target {
                continue;
            }

            //resultに対する挿入
            result.push(SingleId::spatial_decode(&single_id_encode)?);
        }

        if result.is_empty() {
            return Ok(None);
        }

        Ok(Some(result))
    }
}
