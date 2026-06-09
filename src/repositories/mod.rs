pub mod database;

use crate::models::{database::DatabaseMetadata, database::table::TableMetadata};
use redb::{ReadTransaction, WriteTransaction};
use std::collections::HashMap;

pub struct KasaneDbRead {
    pub read_txn: ReadTransaction,
}

impl KasaneDbRead {
    pub fn new(read_txn: ReadTransaction) -> Self {
        Self { read_txn }
    }
}

pub struct KasaneDbWrite {
    pub write_txn: WriteTransaction,
    pub database_caches: HashMap<String, DatabaseMetadata>,
    pub table_caches: HashMap<(uuid::Uuid, String), TableMetadata>,
}

impl KasaneDbWrite {
    pub fn new(write_txn: WriteTransaction) -> Self {
        Self {
            write_txn,
            database_caches: HashMap::new(),
            table_caches: HashMap::new(),
        }
    }

    pub fn commit(self) -> Result<(), crate::error::AppError> {
        self.write_txn.commit()?;
        Ok(())
    }

    pub fn abort(self) -> Result<(), crate::error::AppError> {
        self.write_txn.abort()?;
        Ok(())
    }
}
