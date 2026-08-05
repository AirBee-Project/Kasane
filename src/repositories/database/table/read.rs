use crate::{error::AppError, models::database::table::Table, repositories::KasaneDbRead};

use heed::BytesDecode;

impl<'a> KasaneDbRead<'a> {
    /// Tableの情報を取得する
    #[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
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
        if let Some(m) = db_tables.get(&self.read_txn, &(db_meta.id, table_name))? {
            Ok(Some(Table {
                id: m.id,
                name: table_name.to_string(),
                data_type: m.data_type,
                constraints: m.constraints,
            }))
        } else {
            Ok(None)
        }
    }

    /// Tableの一覧を取得する
    #[tracing::instrument(skip_all, fields(db_name = %db_name))]
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
            let m = heed::types::SerdeJson::<crate::models::database::table::TableMetadata>::bytes_decode(v_bytes).unwrap();
            tables.push(Table {
                id: m.id,
                name: name.to_string(),
                data_type: m.data_type,
                constraints: m.constraints,
            });
        }
        Ok(tables)
    }

    /// テーブルが保持する [`FlexId`](kasane_logic::FlexId) の総数を返す。
    #[tracing::instrument(skip_all)]
    pub fn table_count(&self, table_id: crate::models::id::TableId) -> Result<u64, AppError> {
        use crate::repositories::database::table::data::shard::ShardEntry;

        let mut total = 0u64;
        for item in self
            .db
            .tables_data
            .remap_key_type::<heed::types::Bytes>()
            .prefix_iter(&self.read_txn, table_id.0.as_bytes())?
        {
            let (_, bytes) = item?;
            if let Some(count) = ShardEntry::leaf_count(bytes)? {
                total += count as u64;
            }
        }
        Ok(total)
    }
}
