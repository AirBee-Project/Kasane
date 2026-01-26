use crate::Result;
use async_trait::async_trait;

pub mod memory;

#[cfg(all(not(target_arch = "wasm32"), feature = "redb"))]
pub mod redb;

#[cfg(all(not(target_arch = "wasm32"), feature = "tikv"))]
pub mod tikv;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait Backend: Sync + Send {
    type ReadTx<'a>: ReadTransaction
    where
        Self: 'a;

    type WriteTx<'a>: WriteTransaction
    where
        Self: 'a;

    async fn new(path: &str) -> Result<Self>
    where
        Self: Sized;

    async fn begin_read(&self) -> Result<Self::ReadTx<'_>>;
    async fn begin_write(&self) -> Result<Self::WriteTx<'_>>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait ReadTransaction {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    async fn show_fields(&self) -> Result<Vec<String>>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait WriteTransaction: ReadTransaction {
    async fn set(&mut self, key: &[u8], value: &[u8]) -> Result<()>;
    async fn create_field(&mut self, field_name: String) -> Result<()>;
    async fn drop_field(&mut self, field_name: String) -> Result<()>;
    async fn rename_field(&mut self, old_field_name: String, new_field_name: String) -> Result<()>;

    async fn commit(self) -> Result<()>
    where
        Self: Sized;

    async fn rollback(self) -> Result<()>
    where
        Self: Sized,
    {
        Ok(())
    }
}
