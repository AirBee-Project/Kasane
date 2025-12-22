use std::path::Path;

use redb::ReadableDatabase;

use crate::{
    error::Error,
    io::Kasane,
    transaction::{read::ReadTxTrait, write::WriteTxTrait},
};

pub struct OnDisk {
    db: redb::Database,
}

pub struct OnDiskWriteTx {
    pub(crate) inner: redb::WriteTransaction,
}

pub struct OnDiskReadTx {
    pub(crate) inner: redb::ReadTransaction,
}

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
