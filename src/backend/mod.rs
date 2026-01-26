use async_trait::async_trait;

pub mod memory;

#[cfg(all(not(target_arch = "wasm32"), feature = "redb"))]
pub mod redb;

#[cfg(all(not(target_arch = "wasm32"), feature = "tikv"))]
pub mod tikv;

// Wasmなら ?Send (スレッドセーフ不要)、Nativeなら Send (スレッドセーフ必須)
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait Backend: Sync + Send {
    type Tx<'a>: Transaction
    where
        Self: 'a;

    async fn new(path: &str) -> anyhow::Result<Self>
    where
        Self: Sized;
    async fn begin_read(&self) -> Self::Tx<'_>;
    async fn begin_write(&self) -> Self::Tx<'_>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait Transaction {
    async fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>>;
    async fn set(&mut self, key: &[u8], value: &[u8]) -> anyhow::Result<()>;
    async fn commit(self) -> anyhow::Result<()>;
}
