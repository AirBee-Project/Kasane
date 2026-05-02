use redb::ReadTransaction;

use crate::{db_init::TABLES, error::AppError, models::table::entity::TableMetadata};

pub struct SpatialDbRead {
    read_txn: ReadTransaction,
}

impl SpatialDbRead {
    pub fn new(read_txn: ReadTransaction) -> Self {
        Self { read_txn: read_txn }
    }

    pub fn table_info(&self, name: &str) -> Result<Option<TableMetadata>, AppError> {
        let redb_tables = self.read_txn.open_table(TABLES)?;
        if let Some(meta_data) = redb_tables.get(name)? {
            Ok(Some(meta_data.value().clone()))
        } else {
            Ok(None)
        }
    }
}
