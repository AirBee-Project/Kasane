use kasane_logic::{SetOnMemory, TableOnMemory};

use crate::Error;

pub mod memory;

pub type FieldId = u64;

#[cfg(all(not(target_arch = "wasm32"), feature = "redb"))]
pub mod redb;

#[cfg(all(not(target_arch = "wasm32"), feature = "tikv"))]
pub mod tikv;

pub trait Backend: Sync + Send {
    type ReadTx<'a>: ReadTransaction
    where
        Self: 'a;

    type WriteTx<'a>: WriteTransaction
    where
        Self: 'a;

    fn new(path: &str) -> Result<Self, Error>
    where
        Self: Sized;

    fn begin_read(&self) -> Result<Self::ReadTx<'_>, Error>;
    fn begin_write(&self) -> Result<Self::WriteTx<'_>, Error>;
}

pub trait ReadTransaction {
    fn show_field_ids(&self) -> Result<Vec<FieldId>, Error>;
    fn select(&self, field_id: FieldId, range: SetOnMemory) -> TableOnMemory<&[u8]>;
}

pub trait WriteTransaction: ReadTransaction {
    fn create_field(&mut self) -> Result<FieldId, Error>;
    fn drop_field(&mut self, field_id: FieldId) -> Result<(), Error>;
    fn insert(&mut self, field_id: FieldId, range: SetOnMemory, value: &[u8]) -> Result<(), Error>;
    fn merge(&mut self, field_id: FieldId, range: SetOnMemory, value: &[u8]) -> Result<(), Error>;
    fn update(&mut self, field_id: FieldId, range: SetOnMemory, value: &[u8]) -> Result<(), Error>;

    fn commit(self) -> Result<(), Error>
    where
        Self: Sized;

    fn rollback(self) -> Result<(), Error>
    where
        Self: Sized,
    {
        Ok(())
    }
}
