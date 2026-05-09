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

        let mut redb_spatial_ids = self.write_txn.open_table(SPATIAL_IDS)?;

        let mut should_remove: Vec<SingleId> = Vec::new();
        let mut should_insert: Vec<(SingleId, Vec<u8>)> = Vec::new();

        for single_id in ids.iter_single_ids() {
            //single_idと完全に等しい位置を調べる
            if let Some(access_guard) =
                Self::overlap_equal(&redb_spatial_ids, layer_meta.id, &single_id)?
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
                Self::overlap_parent(&redb_spatial_ids, layer_meta.id, &single_id)?
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
                Self::overlap_children(&redb_spatial_ids, layer_meta.id, &single_id)?
            {
                should_remove.extend(children_single_ids);
            }

            // 一切の重なりがないので普通に挿入する
            should_insert.push((single_id.clone(), data.to_vec()));
        }

        for single_id in should_remove {
            Self::remove(&mut redb_spatial_ids, layer_meta.id, &single_id)?;
        }

        for (single_id, value) in should_insert {
            Self::insert_and_merge(&mut redb_spatial_ids, layer_meta.id, &single_id, &value)?;
        }

        Ok(())
    }

    /// 入力した `SingleId` を挿入する。
    /// merge 可能な場合は siblings を親IDへ集約する。
    pub(self) fn insert_and_merge(
        redb_spatial_ids: &mut Table<'_, ([u8; 16], [u8; 12]), &'static [u8]>,
        layer_id: uuid::Uuid,
        target: &SingleId,
        value: &[u8],
    ) -> Result<(), AppError> {
        let Some(siblings) = target.spatial_siblings() else {
            redb_spatial_ids.insert((layer_id.into_bytes(), target.spatial_encode()), value)?;
            return Ok(());
        };

        // すべてのsiblingsが存在し、かつ同じ値を持つかをチェック
        let can_merge = siblings.iter().all(|sibling| {
            match redb_spatial_ids.get((layer_id.into_bytes(), sibling.spatial_encode())) {
                Ok(Some(access_guard)) => access_guard.value() == value,
                _ => false,
            }
        });

        if !can_merge {
            redb_spatial_ids.insert((layer_id.into_bytes(), target.spatial_encode()), value)?;
            return Ok(());
        }

        // siblings を削除
        for sibling in &siblings {
            Self::remove(redb_spatial_ids, layer_id, sibling)?;
        }

        // 親へ集約
        let parent = target.spatial_parent_at_zoom(target.z() - 1)?;

        redb_spatial_ids.insert((layer_id.into_bytes(), parent.spatial_encode()), value)?;

        Ok(())
    }

    ///入力された[SingleId]を削除する
    pub(self) fn remove<'a>(
        redb_spatial_ids: &'a mut Table<'_, ([u8; 16], [u8; 12]), &'static [u8]>,
        layer_id: uuid::Uuid,
        target: &SingleId,
    ) -> Result<(), AppError> {
        redb_spatial_ids.remove((layer_id.into_bytes(), target.spatial_encode()))?;
        Ok(())
    }

    ///入力された[SingleId]と同じかつ、[SingleId]が存在するかを検証する
    ///
    /// 存在した場合には値の参照を返す
    pub(self) fn overlap_equal<'a>(
        redb_spatial_ids: &'a Table<'_, ([u8; 16], [u8; 12]), &'static [u8]>,
        layer_id: uuid::Uuid,
        target: &SingleId,
    ) -> Result<Option<AccessGuard<'a, &'static [u8]>>, AppError> {
        if let Some(access_guard) = redb_spatial_ids.get((layer_id.into_bytes(), target.spatial_encode()))? {
            return Ok(Some(access_guard));
        }
        Ok(None)
    }

    ///入力された[SingleId]の親となる[SingleId]が存在するかを確かめる
    ///
    /// 存在した場合には[SingleId]と値の参照を返す
    pub(self) fn overlap_parent<'a>(
        redb_spatial_ids: &'a Table<'_, ([u8; 16], [u8; 12]), &'static [u8]>,
        layer_id: uuid::Uuid,
        target: &SingleId,
    ) -> Result<Option<(SingleId, AccessGuard<'a, &'static [u8]>)>, AppError> {
        for parent in target.spatial_parents() {
            if let Some(access_guard) = redb_spatial_ids.get((layer_id.into_bytes(), parent.spatial_encode()))? {
                return Ok(Some((parent, access_guard)));
            }
        }
        Ok(None)
    }

    /// 入力した[SingleId]に含まれる[SingleId]が存在するかを確かめる
    ///
    /// 存在した場合には[SingleId]と値の参照を返す
    pub(self) fn overlap_children(
        redb_spatial_ids: &Table<'_, ([u8; 16], [u8; 12]), &'static [u8]>,
        layer_id: uuid::Uuid,
        target: &SingleId,
    ) -> Result<Option<Vec<SingleId>>, AppError> {
        let mut result: Vec<_> = Vec::new();

        for ele in redb_spatial_ids.range(
            (layer_id.into_bytes(), target.spatial_encode())..=(layer_id.into_bytes(), target.spatial_encode_prefix_max()),
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
