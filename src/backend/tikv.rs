#![cfg(all(not(target_arch = "wasm32"), feature = "tikv"))]

use super::{Backend, ReadTransaction, WriteTransaction};
use crate::Result;
use async_trait::async_trait;
use tikv_client::{Transaction as TikvInnerTx, TransactionClient};

pub struct TikvBackend {
    client: TransactionClient,
}

#[async_trait]
impl Backend for TikvBackend {
    type ReadTx<'a> = TikvReadTx;
    type WriteTx<'a> = TikvWriteTx;

    async fn new(path: &str) -> Result<Self> {
        let client = TransactionClient::new(vec![path]).await?;
        Ok(Self { client })
    }

    async fn begin_read(&self) -> Result<Self::ReadTx<'_>> {
        let txn = self.client.begin_optimistic().await?;
        Ok(TikvReadTx(txn))
    }

    async fn begin_write(&self) -> Result<Self::WriteTx<'_>> {
        let txn = self.client.begin_optimistic().await?;
        Ok(TikvWriteTx(txn))
    }
}

// --- Read Transaction ---
pub struct TikvReadTx(TikvInnerTx);

#[async_trait]
impl ReadTransaction for TikvReadTx {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.0.get(key.to_vec()).await?)
    }
}

// --- Write Transaction ---
pub struct TikvWriteTx(TikvInnerTx);

#[async_trait]
impl ReadTransaction for TikvWriteTx {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.0.get(key.to_vec()).await?)
    }
}

#[async_trait]
impl WriteTransaction for TikvWriteTx {
    async fn set(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        self.0.put(key.to_vec(), value.to_vec()).await?;
        Ok(())
    }

    async fn commit(mut self) -> Result<()> {
        self.0.commit().await?;
        Ok(())
    }

    async fn rollback(mut self) -> Result<()> {
        self.0.rollback().await?;
        Ok(())
    }
}
