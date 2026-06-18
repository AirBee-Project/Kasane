use crate::{error::AppError, models::database::table::Table, repositories::KasaneDbRead};

use heed::BytesDecode;

impl<'a> KasaneDbRead<'a> {
    /// Tableの情報を取得する
    pub fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        if db_name.is_empty() {
            return Ok(None);
        }
        let db_meta = {
            let db = self.db.databases;
            if let Some(m) = db.get(&self.read_txn, db_name)? {
                m
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        let db_tables = self.db.tables;
        if let Some(m) = db_tables.get(&self.read_txn, &(db_meta.id.into_bytes(), table_name))? {
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

    /// Tableの件数を取得する
    pub fn table_count(&self, table_id: uuid::Uuid) -> Result<u64, AppError> {
        let table_id_bytes = table_id.into_bytes();
        let db = self
            .db
            .spatialid_to_value
            .remap_key_type::<heed::types::Bytes>();
        let mut count = 0;
        for iter in db.prefix_iter(&self.read_txn, table_id_bytes.as_slice())? {
            let _ = iter?;
            count += 1;
        }
        Ok(count)
    }

    /// Tableの一覧を取得する
    pub fn table_list(&self, db_name: &str) -> Result<Vec<Table>, AppError> {
        if db_name.is_empty() {
            return Err(AppError::DatabaseNotFound {
                name: db_name.to_string(),
            });
        }
        let db_meta = {
            let db = self.db.databases;
            if let Some(m) = db.get(&self.read_txn, db_name)? {
                m
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        let db_id_bytes = db_meta.id.into_bytes();
        let db = self.db.tables;

        let mut tables = Vec::new();
        for iter in db
            .remap_types::<heed::types::Bytes, heed::types::Bytes>()
            .prefix_iter(&self.read_txn, db_id_bytes.as_slice())?
        {
            let (k_bytes, v_bytes) = iter?;
            let (_, name) = crate::db_init::DbIdAndName::bytes_decode(k_bytes).unwrap();
            let m = heed::types::SerdeBincode::<crate::models::database::table::TableMetadata>::bytes_decode(v_bytes).unwrap();
            tables.push(Table {
                id: m.id,
                name: name.to_string(),
                data_type: m.data_type,
                max_zoom_level: m.max_zoom_level,
            });
        }
        Ok(tables)
    }
}
