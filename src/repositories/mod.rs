pub mod database;
pub mod users;

use crate::models::{database::DatabaseMetadata, database::table::TableMetadata};

use std::collections::HashMap;

pub struct KasaneDbRead<'a> {
    pub read_txn: heed::RoTxn<'a, heed::WithoutTls>,
    pub db: &'a crate::db_init::AppDb,
}

impl<'a> KasaneDbRead<'a> {
    pub fn new(read_txn: heed::RoTxn<'a, heed::WithoutTls>, db: &'a crate::db_init::AppDb) -> Self {
        Self { read_txn, db }
    }
}

pub struct KasaneDbWrite<'a> {
    pub write_txn: heed::RwTxn<'a>,
    pub db: &'a crate::db_init::AppDb,
    pub database_caches: HashMap<String, DatabaseMetadata>,
    pub table_caches: HashMap<(uuid::Uuid, String), TableMetadata>,
}

impl<'a> KasaneDbWrite<'a> {
    pub fn new(write_txn: heed::RwTxn<'a>, db: &'a crate::db_init::AppDb) -> Self {
        Self {
            write_txn,
            db,
            database_caches: HashMap::new(),
            table_caches: HashMap::new(),
        }
    }

    pub fn commit(self) -> Result<(), crate::error::AppError> {
        self.write_txn.commit()?;
        Ok(())
    }

    pub fn abort(self) -> Result<(), crate::error::AppError> {
        self.write_txn.abort();
        Ok(())
    }
}
