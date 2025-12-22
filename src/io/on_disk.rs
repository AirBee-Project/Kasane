use crate::{
    error::Error,
    io::Kasane,
    transaction::{read::ReadTxTrait, write::WriteTxTrait},
};
use redb::{ReadableDatabase, TableDefinition};
use std::path::Path;

pub struct OnDisk {
    pub(crate) db: redb::Database,
}

pub struct OnDiskWriteTx {
    pub(crate) inner: redb::WriteTransaction,
}

pub struct OnDiskReadTx {
    pub(crate) inner: redb::ReadTransaction,
}

static KEY_TABLE: TableDefinition<String, u64> = TableDefinition::new("key");

impl Kasane for OnDisk {
    fn new(path: &Path) -> Result<OnDisk, Error> {
        use redb::Database;
        let db = Database::create(path)?;
        Ok(OnDisk { db })
    }

    fn write_begin(&'_ mut self) -> Result<impl WriteTxTrait, Error> {
        let tx = self.db.begin_write()?;
        Ok(OnDiskWriteTx { inner: tx })
    }

    fn read_begin(&'_ self) -> Result<impl ReadTxTrait, Error> {
        let tx = self.db.begin_read()?;
        Ok(OnDiskReadTx { inner: tx })
    }
}
