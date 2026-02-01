#![cfg(all(not(target_arch = "wasm32"), feature = "redb"))]
use super::{Backend, ReadTransaction, WriteTransaction};
use crate::{backend::FieldId, Result};
use kasane_logic::{SetOnMemory, TableOnMemory};
use redb::{Database, ReadableDatabase, TableDefinition};

const TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("main");

pub struct RedbBackend {
    db: Database,
}

pub struct RedbReadTx(redb::ReadTransaction);
pub struct RedbWriteTx(redb::WriteTransaction);

impl Backend for RedbBackend {
    type ReadTx<'a> = RedbReadTx;
    type WriteTx<'a> = RedbWriteTx;

    fn new(path: &str) -> Result<Self> {
        let db = Database::create(path)?;
        let txn = db.begin_write()?;
        {
            txn.open_table(TABLE)?;
        }
        txn.commit()?;
        Ok(Self { db })
    }

    fn begin_read(&self) -> Result<Self::ReadTx<'_>> {
        let txn = self.db.begin_read()?;
        Ok(RedbReadTx(txn))
    }

    fn begin_write(&self) -> Result<Self::WriteTx<'_>> {
        let txn = self.db.begin_write()?;
        Ok(RedbWriteTx(txn))
    }
}

impl ReadTransaction for RedbReadTx {
    fn show_field_ids(&self) -> Result<Vec<FieldId>> {
        todo!()
    }
    fn select(&self, field_id: FieldId, range: SetOnMemory) -> TableOnMemory<Vec<u8>> {
        todo!()
    }
}

impl ReadTransaction for RedbWriteTx {
    fn show_field_ids(&self) -> Result<Vec<FieldId>> {
        todo!()
    }

    fn select(&self, field_id: FieldId, range: SetOnMemory) -> TableOnMemory<Vec<u8>> {
        todo!()
    }
}

impl WriteTransaction for RedbWriteTx {
    fn create_field(&mut self) -> Result<FieldId> {
        todo!()
    }
    fn drop_field(&mut self, field_id: FieldId) -> Result<()> {
        todo!()
    }
    fn insert(&mut self, field_id: FieldId, range: SetOnMemory, value: Vec<u8>) -> Result<()> {
        todo!()
    }
    fn merge(&mut self, field_id: FieldId, range: SetOnMemory, value: Vec<u8>) -> Result<()> {
        todo!()
    }
    fn update(&mut self, field_id: FieldId, range: SetOnMemory, value: Vec<u8>) -> Result<()> {
        todo!()
    }

    fn commit(self) -> Result<()>
    where
        Self: Sized,
    {
        todo!()
    }

    fn rollback(self) -> Result<()>
    where
        Self: Sized,
    {
        Ok(())
    }
}
