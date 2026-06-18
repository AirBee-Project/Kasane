use kasane_logic::{IntoSingleIds, IterSingleIds, SingleId, SpatialIdSet};

use crate::{error::AppError, repositories::KasaneDbWrite};

type SpatialDataResult<'a> = Result<Option<(SingleId, &'a [u8])>, AppError>;

impl<'a> KasaneDbWrite<'a> {
    pub fn data_insert(
        &mut self,
        table_id: uuid::Uuid,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let mut should_remove: Vec<SingleId> = Vec::new();
        let mut should_insert: Vec<(SingleId, Vec<u8>)> = Vec::new();

        for single_id in ids.iter_single_ids() {
            if let Some(existing_data) = Self::overlap_equal(
                &self.db.spatialid_to_value,
                &self.write_txn,
                table_id,
                &single_id,
            )? {
                if existing_data == data {
                    continue;
                } else {
                    should_insert.push((single_id, data.to_vec()));
                    continue;
                }
            };

            if let Some((parent_single_id, existing_data)) = Self::overlap_parent(
                &self.db.spatialid_to_value,
                &self.write_txn,
                table_id,
                &single_id,
            )? {
                if existing_data == data {
                    continue;
                } else {
                    let overlap_data = existing_data.to_vec();
                    for fragment_single_id in parent_single_id.difference(&single_id) {
                        should_insert.push((fragment_single_id, overlap_data.clone()));
                    }
                    should_insert.push((single_id.clone(), data.to_vec()));
                    should_remove.push(parent_single_id.clone());
                    continue;
                }
            }

            if let Some(children_single_ids) = Self::overlap_children(
                &self.db.spatialid_to_value,
                &self.write_txn,
                table_id,
                &single_id,
            )? {
                should_remove.extend(children_single_ids);
            }

            should_insert.push((single_id.clone(), data.to_vec()));
        }

        for single_id in should_remove {
            Self::remove(&mut self.write_txn, self.db, table_id, &single_id)?;
        }

        for (single_id, value) in should_insert {
            Self::insert_and_merge(&mut self.write_txn, self.db, table_id, &single_id, &value)?;
        }

        Ok(())
    }

    pub fn data_upsert(
        &mut self,
        table_id: uuid::Uuid,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let mut should_insert: Vec<(SingleId, Vec<u8>)> = Vec::new();

        for single_id in ids.iter_single_ids() {
            if Self::overlap_equal(
                &self.db.spatialid_to_value,
                &self.write_txn,
                table_id,
                &single_id,
            )? == Some(data)
            {
                continue;
            }

            if let Some(children_single_ids) = Self::overlap_children(
                &self.db.spatialid_to_value,
                &self.write_txn,
                table_id,
                &single_id,
            )? {
                let mut children_set = SpatialIdSet::new();
                let mut parent_set = SpatialIdSet::new();

                for sid in children_single_ids {
                    children_set.insert(sid);
                }

                parent_set.insert(single_id);

                let insert_set = parent_set - children_set;

                should_insert.extend(insert_set.into_single_ids().map(|sid| (sid, data.to_vec())));

                continue;
            }

            should_insert.push((single_id.clone(), data.to_vec()));
        }

        for (single_id, value) in should_insert {
            Self::insert_and_merge(&mut self.write_txn, self.db, table_id, &single_id, &value)?;
        }

        Ok(())
    }

    pub fn data_remove(&mut self, table_id: uuid::Uuid, ids: SpatialIdSet) -> Result<(), AppError> {
        let mut should_remove: Vec<SingleId> = Vec::new();
        let mut should_insert: Vec<(SingleId, Vec<u8>)> = Vec::new();

        for single_id in ids.iter_single_ids() {
            if Self::overlap_equal(
                &self.db.spatialid_to_value,
                &self.write_txn,
                table_id,
                &single_id,
            )?
            .is_some()
            {
                should_remove.push(single_id);
                continue;
            };

            if let Some((parent_single_id, existing_data)) = Self::overlap_parent(
                &self.db.spatialid_to_value,
                &self.write_txn,
                table_id,
                &single_id,
            )? {
                let overlap_data = existing_data.to_vec();
                for fragment_single_id in parent_single_id.difference(&single_id) {
                    should_insert.push((fragment_single_id, overlap_data.clone()));
                }
                should_remove.push(parent_single_id.clone());
                continue;
            }

            if let Some(children_single_ids) = Self::overlap_children(
                &self.db.spatialid_to_value,
                &self.write_txn,
                table_id,
                &single_id,
            )? {
                should_remove.extend(children_single_ids);
            }
        }

        for single_id in should_remove {
            Self::remove(&mut self.write_txn, self.db, table_id, &single_id)?;
        }

        for (single_id, value) in should_insert {
            Self::insert_and_merge(&mut self.write_txn, self.db, table_id, &single_id, &value)?;
        }

        Ok(())
    }

    pub(self) fn insert_and_merge(
        write_txn: &mut heed::RwTxn<'a>,
        db: &crate::db_init::AppDb,
        table_id: uuid::Uuid,
        target: &SingleId,
        value: &[u8],
    ) -> Result<(), AppError> {
        let mut current_target = target.clone();

        loop {
            let Some(siblings) = current_target.spatial_siblings() else {
                db.spatialid_to_value.put(
                    write_txn,
                    &(table_id.into_bytes(), current_target.spatial_encode()),
                    value,
                )?;
                db.value_to_spatialid.put(
                    write_txn,
                    &(
                        table_id.into_bytes(),
                        value,
                        current_target.spatial_encode(),
                    ),
                    &(),
                )?;
                return Ok(());
            };

            let mut can_merge = true;
            for sibling in &siblings {
                if sibling == &current_target {
                    continue;
                }
                match db.spatialid_to_value.get(
                    write_txn,
                    &(table_id.into_bytes(), sibling.spatial_encode()),
                ) {
                    Ok(Some(existing_data)) => {
                        if existing_data != value {
                            can_merge = false;
                            break;
                        }
                    }
                    _ => {
                        can_merge = false;
                        break;
                    }
                }
            }

            if !can_merge {
                db.spatialid_to_value.put(
                    write_txn,
                    &(table_id.into_bytes(), current_target.spatial_encode()),
                    value,
                )?;
                db.value_to_spatialid.put(
                    write_txn,
                    &(
                        table_id.into_bytes(),
                        value,
                        current_target.spatial_encode(),
                    ),
                    &(),
                )?;
                return Ok(());
            }

            for sibling in &siblings {
                Self::remove(write_txn, db, table_id, sibling)?;
            }

            current_target = current_target.spatial_parent_at_zoom(current_target.z() - 1)?;
        }
    }

    pub(self) fn remove(
        write_txn: &mut heed::RwTxn<'a>,
        db: &crate::db_init::AppDb,
        table_id: uuid::Uuid,
        target: &SingleId,
    ) -> Result<(), AppError> {
        let existing_data = db
            .spatialid_to_value
            .get(write_txn, &(table_id.into_bytes(), target.spatial_encode()))?
            .map(|val| val.to_vec());

        if let Some(existing_data) = existing_data {
            db.value_to_spatialid.delete(
                write_txn,
                &(
                    table_id.into_bytes(),
                    &existing_data,
                    target.spatial_encode(),
                ),
            )?;
            db.spatialid_to_value
                .delete(write_txn, &(table_id.into_bytes(), target.spatial_encode()))?;
        }
        Ok(())
    }

    pub(self) fn overlap_equal<'txn>(
        spatial_db: &heed::Database<crate::db_init::TableIdAndSpatialId, heed::types::Bytes>,
        txn: &'txn heed::RwTxn<'a>,
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
        txn: &'txn heed::RwTxn<'a>,
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

    pub(self) fn overlap_children(
        spatial_db: &heed::Database<crate::db_init::TableIdAndSpatialId, heed::types::Bytes>,
        txn: &heed::RwTxn<'a>,
        table_id: uuid::Uuid,
        target: &SingleId,
    ) -> Result<Option<Vec<SingleId>>, AppError> {
        let mut result = Vec::new();

        let start = (table_id.into_bytes(), target.spatial_encode());
        let end = (table_id.into_bytes(), target.spatial_encode_prefix_max());

        for item in spatial_db.range(txn, &(start..=end))? {
            let (key, _) = item?;
            let single_id_encode = key.1;

            if SingleId::spatial_decode(&single_id_encode)? == *target {
                continue;
            }

            result.push(SingleId::spatial_decode(&single_id_encode)?);
        }

        if result.is_empty() {
            return Ok(None);
        }

        Ok(Some(result))
    }
}
