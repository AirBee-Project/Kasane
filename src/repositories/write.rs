use redb::{ReadableTable, WriteTransaction};

use crate::{db_init::TABLES, error::AppError, models::table::entity::TableMetadata};

pub struct SpatialDbWrite {
    write_txn: WriteTransaction,
}

impl SpatialDbWrite {
    pub fn new(write_txn: WriteTransaction) -> Self {
        Self { write_txn }
    }

    pub fn table_info(&self, name: &str) -> Result<Option<TableMetadata>, AppError> {
        let redb_tables = self.write_txn.open_table(TABLES)?;
        if let Some(meta_data) = redb_tables.get(name)? {
            Ok(Some(meta_data.value().clone()))
        } else {
            Ok(None)
        }
    }

    ///KasaneのTableを作成する
    ///既存のTableとの重複確認は行わない
    pub fn table_create(&self, name: &str, meta_data: TableMetadata) -> Result<(), AppError> {
        let mut redb_tables = self.write_txn.open_table(TABLES)?;
        let _ = redb_tables.insert(name, meta_data)?;
        return Ok(());
    }

    pub fn table_remove(&self, name: &str) -> Result<(), AppError> {
        let mut redb_tables = self.write_txn.open_table(TABLES)?;
        let removed = redb_tables.remove(name)?;

        if removed.is_none() {
            return Err(AppError::TableNotFound {
                name: name.to_string(),
            });
        }

        Ok(())
    }

    pub fn commit(self) -> Result<(), AppError> {
        self.write_txn.commit()?;
        Ok(())
    }
}
