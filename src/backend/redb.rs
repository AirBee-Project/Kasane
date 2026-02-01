#![cfg(all(not(target_arch = "wasm32"), feature = "redb"))]
use super::{Backend, ReadTransaction, WriteTransaction};
use crate::{backend::FieldId, Error};
use kasane_logic::{SetOnMemory, TableOnMemory};
use redb::{Database, ReadableDatabase, TableDefinition};

const MAIN: TableDefinition<&[u8], &[u8]> = TableDefinition::new("main");
const FIELD: TableDefinition<&[u8], &[u8]> = TableDefinition::new("main");

pub struct RedbBackend {
    db: Database,
}

pub struct RedbReadTx(redb::ReadTransaction);
pub struct RedbWriteTx(redb::WriteTransaction);

impl Backend for RedbBackend {
    type ReadTx<'a> = RedbReadTx;
    type WriteTx<'a> = RedbWriteTx;

    fn new(path: &str) -> Result<Self, Error> {
        let db = Database::create(path)?;
        let txn = db.begin_write()?;
        {
            txn.open_table(MAIN)?;
            txn.open_table(FIELD)?;
        }
        txn.commit()?;
        Ok(Self { db })
    }

    fn begin_read(&self) -> Result<Self::ReadTx<'_>, Error> {
        let txn = self.db.begin_read()?;
        Ok(RedbReadTx(txn))
    }

    fn begin_write(&self) -> Result<Self::WriteTx<'_>, Error> {
        let txn = self.db.begin_write()?;
        Ok(RedbWriteTx(txn))
    }
}

impl ReadTransaction for RedbReadTx {
    fn show_field_ids(&self) -> Result<Vec<FieldId>, Error> {
        todo!()
    }
    fn select(&self, field_id: FieldId, range: SetOnMemory) -> TableOnMemory<&[u8]> {
        todo!()
    }
}

impl ReadTransaction for RedbWriteTx {
    fn show_field_ids(&self) -> Result<Vec<FieldId>, Error> {
        todo!()
    }

    fn select(&self, field_id: FieldId, range: SetOnMemory) -> TableOnMemory<&[u8]> {
        todo!()
    }
}

impl WriteTransaction for RedbWriteTx {
    fn create_field(&mut self) -> Result<FieldId, Error> {
        todo!()
    }
    fn drop_field(&mut self, field_id: FieldId) -> Result<(), Error> {
        todo!()
    }
    fn insert(&mut self, field_id: FieldId, range: SetOnMemory, value: &[u8]) -> Result<(), Error> {
        todo!()
    }
    fn merge(&mut self, field_id: FieldId, range: SetOnMemory, value: &[u8]) -> Result<(), Error> {
        todo!()
    }
    fn update(&mut self, field_id: FieldId, range: SetOnMemory, value: &[u8]) -> Result<(), Error> {
        todo!()
    }

    fn commit(self) -> Result<(), Error>
    where
        Self: Sized,
    {
        self.0.commit();
        Ok(())
    }

    fn rollback(self) -> Result<(), Error>
    where
        Self: Sized,
    {
        self.0.abort();
        Ok(())
    }
}
