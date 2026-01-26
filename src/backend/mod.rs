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

    async fn new(path: &str) -> anyhow::Result<Self>
    where
        Self: Sized;

    // ▼▼▼ 修正: anyhow::Result で包む ▼▼▼
    async fn begin_read(&self) -> anyhow::Result<Self::ReadTx<'_>>;
    async fn begin_write(&self) -> anyhow::Result<Self::WriteTx<'_>>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait ReadTransaction {
    async fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait WriteTransaction: ReadTransaction {
    async fn set(&mut self, key: &[u8], value: &[u8]) -> anyhow::Result<()>;

    // where Self: Sized を追加 (前回の修正点)
    async fn commit(self) -> anyhow::Result<()>
    where
        Self: Sized;

    async fn rollback(self) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        Ok(())
    }
}
