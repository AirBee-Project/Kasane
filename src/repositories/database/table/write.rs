use uuid::Uuid;

use crate::{
    error::AppError,
    models::database::table::{Table, TableDataType, TableMetadata},
    repositories::KasaneDbWrite,
};

use heed::BytesDecode;

impl<'a> KasaneDbWrite<'a> {
    /// Tableの情報を取得する
    pub fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        if db_name.is_empty() {
            return Ok(None);
        }
        let db_meta = {
            let db = self.db.databases;
            if let Some(m) = db.get(&self.write_txn, db_name)? {
                m
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        if let Some(meta_data) = self.table_caches.get(&(db_meta.id, table_name.to_string())) {
            return Ok(Some(Table {
                id: meta_data.id,
                name: table_name.to_string(),
                data_type: meta_data.data_type,
                max_zoom_level: meta_data.max_zoom_level,
            }));
        }

        let db = self.db.tables;
        if let Some(m) = db.get(&self.write_txn, &(db_meta.id.into_bytes(), table_name))? {
            Ok(Some(Table {
                id: m.id,
                name: table_name.to_string(),
                data_type: m.data_type,
                max_zoom_level: m.max_zoom_level,
            }))
        } else {
            Ok(None)
        }
    }

    /// Tableを作成する
    pub fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
    ) -> Result<Table, AppError> {
        if db_name.is_empty() {
            return Err(AppError::DatabaseNotFound {
                name: db_name.to_string(),
            });
        }
        if table_name.is_empty() {
            return Err(AppError::InternalError(
                "Table name cannot be empty".to_string(),
            ));
        }
        let db_meta = {
            let db = self.db.databases;
            if let Some(m) = db.get(&self.write_txn, db_name)? {
                m
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        if self.table_info(db_name, table_name)?.is_some() {
            return Err(AppError::TableAlreadyExists {
                name: table_name.to_string(),
            });
        }

        let db_index = self.db.table_id_index;

        let mut id = Uuid::now_v7();
        loop {
            if db_index.get(&self.write_txn, &id.into_bytes())?.is_none() {
                break;
            }
            id = Uuid::now_v7();
        }

        let meta = TableMetadata {
            id,
            data_type,
            max_zoom_level,
        };

        let db = self.db.tables;
        db.put(
            &mut self.write_txn,
            &(db_meta.id.into_bytes(), table_name),
            &meta,
        )?;
        db_index.put(&mut self.write_txn, &id.into_bytes(), &())?;

        self.table_caches
            .insert((db_meta.id, table_name.to_string()), meta);

        Ok(Table {
            id,
            name: table_name.to_string(),
            data_type,
            max_zoom_level,
        })
    }

    /// Tableを削除する
    pub fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError> {
        if db_name.is_empty() {
            return Err(AppError::DatabaseNotFound {
                name: db_name.to_string(),
            });
        }
        if table_name.is_empty() {
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        }
        let db_meta = {
            let db = self.db.databases;
            if let Some(m) = db.get(&self.write_txn, db_name)? {
                m
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        let table_meta = if let Some(meta) = self.table_info(db_name, table_name)? {
            meta
        } else {
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        };

        let db = self.db.tables;
        db.delete(&mut self.write_txn, &(db_meta.id.into_bytes(), table_name))?;

        let db_index = self.db.table_id_index;
        db_index.delete(&mut self.write_txn, &table_meta.id.into_bytes())?;

        let table_id_bytes = table_meta.id.into_bytes();

        let db_spatial = self.db.spatialid_to_value;
        let mut keys_to_delete = Vec::new();
        for iter in db_spatial
            .remap_key_type::<heed::types::Bytes>()
            .prefix_iter(&self.write_txn, table_id_bytes.as_slice())?
        {
            let (k, _) = iter?;
            let decoded_k = crate::db_init::TableIdAndSpatialId::bytes_decode(k).unwrap();
            keys_to_delete.push(decoded_k);
        }
        for k in keys_to_delete {
            db_spatial.delete(&mut self.write_txn, &k)?;
        }

        let db_value = self.db.value_to_spatialid;
        let mut v_keys_to_delete = Vec::new();
        for iter in db_value
            .remap_key_type::<heed::types::Bytes>()
            .prefix_iter(&self.write_txn, table_id_bytes.as_slice())?
        {
            let (k, _) = iter?;
            let decoded_k = crate::db_init::ValueToSpatialId::bytes_decode(k).unwrap();
            v_keys_to_delete.push((decoded_k.0, decoded_k.1.to_vec(), decoded_k.2));
        }
        for (id, val, spatial) in v_keys_to_delete {
            db_value.delete(&mut self.write_txn, &(id, &val, spatial))?;
        }

        self.table_caches
            .remove(&(db_meta.id, table_name.to_string()));

        Ok(())
    }
}
