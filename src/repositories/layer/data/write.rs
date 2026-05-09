use kasane_logic::{IntoSingleIds, IterSingleIds, SingleId, SpatialIdSet};
use redb::{AccessGuard, ReadableTable, Table};

use crate::{
    db_init::{SPATIALID_TO_VALUE, VALUE_TO_SPATIALID},
    error::AppError,
    repositories::layer::write::SpatialDbWrite,
};

type SpatialDataResult<'a> = Result<Option<(SingleId, AccessGuard<'a, &'static [u8]>)>, AppError>;

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

        let mut redb_sppatialid_to_value = self.write_txn.open_table(SPATIALID_TO_VALUE)?;

        let mut should_remove: Vec<SingleId> = Vec::new();
        let mut should_insert: Vec<(SingleId, Vec<u8>)> = Vec::new();

        for single_id in ids.iter_single_ids() {
            //single_idと完全に等しい位置を調べる
            if let Some(access_guard) =
                Self::overlap_equal(&redb_sppatialid_to_value, layer_meta.id, &single_id)?
            {
                if access_guard.value() == data {
                    continue;
                } else {
                    //完全に等しいので上書きだけで良い
                    should_insert.push((single_id, data.to_vec()));
                    continue;
                }
            };

            //single_idの親を調べていく
            if let Some((parent_single_id, access_guard)) =
                Self::overlap_parent(&redb_sppatialid_to_value, layer_meta.id, &single_id)?
            {
                if access_guard.value() == data {
                    //値が等しいので何もしなくてよい
                    continue;
                } else {
                    let overlap_data = access_guard.value().to_vec();
                    for fragment_single_id in parent_single_id.difference(&single_id) {
                        should_insert.push((fragment_single_id, overlap_data.clone()));
                    }
                    should_insert.push((single_id.clone(), data.to_vec()));
                    should_remove.push(parent_single_id.clone());
                    continue;
                }
            }

            //single_idの子を全て削除する
            if let Some(children_single_ids) =
                Self::overlap_children(&redb_sppatialid_to_value, layer_meta.id, &single_id)?
            {
                should_remove.extend(children_single_ids);
            }

            // 一切の重なりがないので普通に挿入する
            should_insert.push((single_id.clone(), data.to_vec()));
        }

        let mut redb_value_to_spatialid = self.write_txn.open_table(VALUE_TO_SPATIALID)?;

        for single_id in should_remove {
            Self::remove(
                &mut redb_sppatialid_to_value,
                &mut redb_value_to_spatialid,
                layer_meta.id,
                &single_id,
            )?;
        }

        for (single_id, value) in should_insert {
            Self::insert_and_merge(
                &mut redb_sppatialid_to_value,
                &mut redb_value_to_spatialid,
                layer_meta.id,
                &single_id,
                &value,
            )?;
        }

        Ok(())
    }

    pub fn data_upsert(
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

        let mut redb_sppatialid_to_value = self.write_txn.open_table(SPATIALID_TO_VALUE)?;

        let mut should_insert: Vec<(SingleId, Vec<u8>)> = Vec::new();

        #[allow(clippy::collapsible_if)]
        for single_id in ids.iter_single_ids() {
            //single_idと完全に等しい位置を調べる
            if let Some(access_guard) =
                Self::overlap_equal(&redb_sppatialid_to_value, layer_meta.id, &single_id)?
            {
                if access_guard.value() == data {
                    continue;
                }
            }

            //single_idの親を調べていく
            if let Some(children_single_ids) =
                Self::overlap_children(&redb_sppatialid_to_value, layer_meta.id, &single_id)?
            {
                let mut children_set = SpatialIdSet::new();
                let mut parent_set = SpatialIdSet::new();

                for single_id in children_single_ids {
                    children_set.insert(single_id);
                }

                parent_set.insert(single_id);

                let insert_set = parent_set - children_set;

                should_insert.extend(
                    insert_set
                        .into_single_ids()
                        .map(|single_id| (single_id, data.to_vec())),
                );

                continue;
            }

            // 一切の重なりがないので普通に挿入する
            should_insert.push((single_id.clone(), data.to_vec()));
        }

        let mut redb_value_to_spatialid = self.write_txn.open_table(VALUE_TO_SPATIALID)?;

        for (single_id, value) in should_insert {
            Self::insert_and_merge(
                &mut redb_sppatialid_to_value,
                &mut redb_value_to_spatialid,
                layer_meta.id,
                &single_id,
                &value,
            )?;
        }

        Ok(())
    }

    /// 入力した `SingleId` を挿入する。
    /// merge 可能な場合は siblings を親IDへ集約する。
    ///
    /// SingleIdの情報のKey-Value Storeへの実際の挿入はこの関数で一元管理する。
    #[allow(clippy::type_complexity)]
    pub(self) fn insert_and_merge(
        redb_sppatialid_to_value: &mut Table<'_, ([u8; 16], [u8; 12]), &'static [u8]>,
        redb_value_to_spatialid: &mut Table<'_, ([u8; 16], &[u8], [u8; 12]), ()>,
        layer_id: uuid::Uuid,
        target: &SingleId,
        value: &[u8],
    ) -> Result<(), AppError> {
        let Some(siblings) = target.spatial_siblings() else {
            redb_sppatialid_to_value
                .insert((layer_id.into_bytes(), target.spatial_encode()), value)?;
            return Ok(());
        };

        // すべてのsiblingsが存在し、かつ同じ値を持つかをチェック
        let can_merge = siblings.iter().all(|sibling| {
            match redb_sppatialid_to_value.get((layer_id.into_bytes(), sibling.spatial_encode())) {
                Ok(Some(access_guard)) => access_guard.value() == value,
                _ => false,
            }
        });

        if !can_merge {
            redb_sppatialid_to_value
                .insert((layer_id.into_bytes(), target.spatial_encode()), value)?;
            return Ok(());
        }

        // siblings を削除
        for sibling in &siblings {
            Self::remove(
                redb_sppatialid_to_value,
                redb_value_to_spatialid,
                layer_id,
                sibling,
            )?;
        }

        // 親へ集約
        let parent = target.spatial_parent_at_zoom(target.z() - 1)?;

        redb_sppatialid_to_value.insert((layer_id.into_bytes(), parent.spatial_encode()), value)?;

        Ok(())
    }

    ///入力された[SingleId]を削除する
    ///
    /// SingleIdの情報のKey－Value Storeからの削除はこの関数で一元管理する。
    #[allow(clippy::type_complexity)]
    pub(self) fn remove<'a>(
        redb_sppatialid_to_value: &'a mut Table<'_, ([u8; 16], [u8; 12]), &'static [u8]>,
        redb_value_to_spatialid: &'a mut Table<'_, ([u8; 16], &[u8], [u8; 12]), ()>,
        layer_id: uuid::Uuid,
        target: &SingleId,
    ) -> Result<(), AppError> {
        if let Some(access_guard) =
            redb_sppatialid_to_value.remove((layer_id.into_bytes(), target.spatial_encode()))?
        {
            let value = access_guard.value();
            redb_value_to_spatialid.remove((
                *layer_id.as_bytes(),
                value,
                target.spatial_encode(),
            ))?;
            Ok(())
        } else {
            Ok(())
        }
    }

    ///入力された[SingleId]と同じかつ、[SingleId]が存在するかを検証する
    ///
    /// 存在した場合には値の参照を返す
    pub(self) fn overlap_equal<'a>(
        redb_sppatialid_to_value: &'a Table<'_, ([u8; 16], [u8; 12]), &'static [u8]>,
        layer_id: uuid::Uuid,
        target: &SingleId,
    ) -> Result<Option<AccessGuard<'a, &'static [u8]>>, AppError> {
        if let Some(access_guard) =
            redb_sppatialid_to_value.get((layer_id.into_bytes(), target.spatial_encode()))?
        {
            return Ok(Some(access_guard));
        }
        Ok(None)
    }

    ///入力された[SingleId]の親となる[SingleId]が存在するかを確かめる
    ///
    /// 存在した場合には[SingleId]と値の参照を返す
    pub(self) fn overlap_parent<'a>(
        redb_sppatialid_to_value: &'a Table<'_, ([u8; 16], [u8; 12]), &'static [u8]>,
        layer_id: uuid::Uuid,
        target: &SingleId,
    ) -> SpatialDataResult<'a> {
        for parent in target.spatial_parents() {
            if let Some(access_guard) =
                redb_sppatialid_to_value.get((layer_id.into_bytes(), parent.spatial_encode()))?
            {
                return Ok(Some((parent, access_guard)));
            }
        }
        Ok(None)
    }

    /// 入力した[SingleId]に含まれる[SingleId]が存在するかを確かめる
    ///
    /// 存在した場合には[SingleId]と値の参照を返す
    pub(self) fn overlap_children(
        redb_sppatialid_to_value: &Table<'_, ([u8; 16], [u8; 12]), &'static [u8]>,
        layer_id: uuid::Uuid,
        target: &SingleId,
    ) -> Result<Option<Vec<SingleId>>, AppError> {
        let mut result: Vec<_> = Vec::new();

        for ele in redb_sppatialid_to_value.range(
            (layer_id.into_bytes(), target.spatial_encode())
                ..=(layer_id.into_bytes(), target.spatial_encode_prefix_max()),
        )? {
            let (key, _) = ele?;
            let (_, single_id_encode) = key.value();

            //equalの排除
            if SingleId::spatial_decode(&single_id_encode)? == *target {
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
