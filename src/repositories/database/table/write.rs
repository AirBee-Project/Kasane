use uuid::Uuid;

use crate::{
    error::AppError,
    models::database::table::{Table, TableDataType, TableMetadata},
    repositories::KasaneDbWrite,
};

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
        if let Some(m) = db.get(&self.write_txn, &(db_meta.id, table_name))? {
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

        let mut id = crate::models::id::TableId(uuid::Uuid::now_v7());
        loop {
            if db_index.get(&self.write_txn, &id)?.is_none() {
                break;
            }
            id = crate::models::id::TableId(uuid::Uuid::now_v7());
        }

        let meta = TableMetadata {
            id,
            data_type,
            max_zoom_level,
        };

        let db = self.db.tables;
        db.put(
            &mut self.write_txn,
            &(db_meta.id, table_name),
            &meta,
        )?;
        db_index.put(&mut self.write_txn, &id, &())?;

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
        todo!()
    }
}
