use crate::Result;
use kasane_logic::{SetOnMemory, TableOnMemory};

pub mod memory;

pub type FieldId = u64;

#[cfg(all(not(target_arch = "wasm32"), feature = "redb"))]
pub mod redb;

#[cfg(all(not(target_arch = "wasm32"), feature = "tikv"))]
pub mod tikv;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Backend: Sync + Send {
    type ReadTx<'a>: ReadTransaction
    where
        Self: 'a;

    type WriteTx<'a>: WriteTransaction
    where
        Self: 'a;

    fn new(path: &str) -> Result<Self>
    where
        Self: Sized;

    fn begin_read(&self) -> Result<Self::ReadTx<'_>>;
    fn begin_write(&self) -> Result<Self::WriteTx<'_>>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ReadTransaction {
    fn show_field_ids(&self) -> Result<Vec<FieldId>>;
    fn select(&self, field_id: FieldId, range: SetOnMemory) -> TableOnMemory<Vec<u8>>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait WriteTransaction: ReadTransaction {
    fn create_field(&mut self) -> Result<FieldId>;
    fn drop_field(&mut self, field_id: FieldId) -> Result<()>;
    fn insert(&mut self, field_id: FieldId, range: SetOnMemory, value: Vec<u8>) -> Result<()>;
    fn merge(&mut self, field_id: FieldId, range: SetOnMemory, value: Vec<u8>) -> Result<()>;
    fn update(&mut self, field_id: FieldId, range: SetOnMemory, value: Vec<u8>) -> Result<()>;

    fn commit(self) -> Result<()>
    where
        Self: Sized;

    fn rollback(self) -> Result<()>
    where
        Self: Sized,
    {
        Ok(())
    }
}
