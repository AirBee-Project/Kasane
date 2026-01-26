#![cfg(not(target_arch = "wasm32"))]

use super::{Backend, Transaction};
use async_trait::async_trait;
use redb::{Database, ReadableDatabase as _, ReadableTable as _, TableDefinition};

const TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("main");

pub struct RedbBackend {
    db: Database,
}

#[async_trait]
impl Backend for RedbBackend {
    // RedbTxにライフタイム 'a を渡す
    type Tx<'a> = RedbTx;

    async fn new(path: &str) -> anyhow::Result<Self> {
        let db = Database::create(path)?;
        let txn = db.begin_write()?;
        {
            txn.open_table(TABLE)?;
        }
        txn.commit()?;
        Ok(Self { db })
    }

    async fn begin_read(&self) -> Self::Tx<'_> {
        let txn = self.db.begin_read().expect("Failed to begin read");
        RedbTx::Read(txn)
    }

    async fn begin_write(&self) -> Self::Tx<'_> {
        let txn = self.db.begin_write().expect("Failed to begin write");
        RedbTx::Write(txn)
    }
}

pub enum RedbTx {
    Read(redb::ReadTransaction),
    Write(redb::WriteTransaction),
}

#[async_trait]
impl<'a> Transaction for RedbTx {
    async fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        let val = match self {
            Self::Read(txn) => {
                let table = txn.open_table(TABLE)?;
                let res = table.get(key)?;
                res.map(|v| v.value().to_vec())
            }
            Self::Write(txn) => {
                let table = txn.open_table(TABLE)?;
                let res = table.get(key)?;
                res.map(|v| v.value().to_vec())
            }
        };
        Ok(val)
    }

    async fn set(&mut self, key: &[u8], value: &[u8]) -> anyhow::Result<()> {
        match self {
            Self::Write(txn) => {
                let mut table = txn.open_table(TABLE)?;
                table.insert(key, value)?;
                Ok(())
            }
            Self::Read(_) => anyhow::bail!("Cannot write to read transaction"),
        }
    }

    async fn commit(self) -> anyhow::Result<()> {
        match self {
            Self::Write(txn) => {
                txn.commit()?;
                Ok(())
            }
            Self::Read(_) => Ok(()),
        }
    }
}
