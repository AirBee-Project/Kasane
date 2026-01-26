#![cfg(all(not(target_arch = "wasm32"), feature = "redb"))]

use super::{Backend, ReadTransaction, WriteTransaction};
use crate::Result;
use async_trait::async_trait;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

const TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("main");

pub struct RedbBackend {
    db: Database,
}

#[async_trait]
impl Backend for RedbBackend {
    type ReadTx<'a> = RedbReadTx;
    type WriteTx<'a> = RedbWriteTx;

    async fn new(path: &str) -> Result<Self> {
        let db = Database::create(path)?;
        let txn = db.begin_write()?;
        {
            txn.open_table(TABLE)?;
        }
        txn.commit()?;
        Ok(Self { db })
    }

    async fn begin_read(&self) -> Result<Self::ReadTx<'_>> {
        let txn = self.db.begin_read()?;
        Ok(RedbReadTx(txn))
    }

    async fn begin_write(&self) -> Result<Self::WriteTx<'_>> {
        let txn = self.db.begin_write()?;
        Ok(RedbWriteTx(txn))
    }
}

// --- Read Transaction ---
pub struct RedbReadTx(redb::ReadTransaction);

#[async_trait]
impl ReadTransaction for RedbReadTx {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let table = self.0.open_table(TABLE)?;
        let res = table.get(key)?;
        Ok(res.map(|v| v.value().to_vec()))
    }
}

// --- Write Transaction ---
pub struct RedbWriteTx(redb::WriteTransaction);

#[async_trait]
impl ReadTransaction for RedbWriteTx {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let table = self.0.open_table(TABLE)?;
        let res = table.get(key)?;
        Ok(res.map(|v| v.value().to_vec()))
    }
}

#[async_trait]
impl WriteTransaction for RedbWriteTx {
    async fn set(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        let mut table = self.0.open_table(TABLE)?;
        table.insert(key, value)?;
        Ok(())
    }

    async fn commit(self) -> Result<()> {
        self.0.commit()?;
        Ok(())
    }

    async fn rollback(self) -> Result<()> {
        self.0.abort()?;
        Ok(())
    }
}
