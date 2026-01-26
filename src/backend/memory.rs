use super::{Backend, ReadTransaction, WriteTransaction};
use async_trait::async_trait;
use std::collections::HashMap;

// --- 環境別のロック機構切り替え ---
#[cfg(not(target_arch = "wasm32"))]
mod types {
    pub use std::sync::{Arc, RwLock};
    pub type DbMap = Arc<RwLock<std::collections::HashMap<Vec<u8>, Vec<u8>>>>;
}

#[cfg(target_arch = "wasm32")]
mod types {
    pub use std::cell::RefCell as RwLock;
    pub use std::rc::Rc as Arc;
    pub type DbMap = Arc<RwLock<std::collections::HashMap<Vec<u8>, Vec<u8>>>>;
}
use types::*;
// --------------------------------

#[derive(Clone)]
pub struct MemoryBackend {
    data: DbMap,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Backend for MemoryBackend {
    type ReadTx<'a> = MemoryReadTx;
    type WriteTx<'a> = MemoryWriteTx;

    async fn new(_path: &str) -> anyhow::Result<Self> {
        Ok(Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    // ▼▼▼ 修正: Resultを返すように変更 ▼▼▼
    async fn begin_read(&self) -> anyhow::Result<Self::ReadTx<'_>> {
        #[cfg(not(target_arch = "wasm32"))]
        let data = self.data.read().expect("Lock poisoned").clone();
        #[cfg(target_arch = "wasm32")]
        let data = self.data.borrow().clone();

        Ok(MemoryReadTx { data })
    }

    async fn begin_write(&self) -> anyhow::Result<Self::WriteTx<'_>> {
        #[cfg(not(target_arch = "wasm32"))]
        let snapshot = self.data.read().expect("Lock poisoned").clone();
        #[cfg(target_arch = "wasm32")]
        let snapshot = self.data.borrow().clone();

        Ok(MemoryWriteTx {
            staging: snapshot,
            target: self.data.clone(),
        })
    }
}

// ... (後略)
// --- Read Transaction ---
pub struct MemoryReadTx {
    data: HashMap<Vec<u8>, Vec<u8>>,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ReadTransaction for MemoryReadTx {
    async fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.data.get(key).cloned())
    }
}

// --- Write Transaction ---
pub struct MemoryWriteTx {
    staging: HashMap<Vec<u8>, Vec<u8>>,
    target: DbMap,
}

// WriteTransactionはReadTransactionを実装する必要がある
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ReadTransaction for MemoryWriteTx {
    async fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        // 自分の作業領域から読む
        Ok(self.staging.get(key).cloned())
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl WriteTransaction for MemoryWriteTx {
    async fn set(&mut self, key: &[u8], value: &[u8]) -> anyhow::Result<()> {
        self.staging.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    async fn commit(self) -> anyhow::Result<()> {
        // ここでロックを取って書き戻す
        #[cfg(not(target_arch = "wasm32"))]
        let mut guard = self
            .target
            .write()
            .map_err(|_| anyhow::anyhow!("Lock poisoned"))?;

        #[cfg(target_arch = "wasm32")]
        let mut guard = self.target.borrow_mut();

        *guard = self.staging;
        Ok(())
    }
}
// ... (前略)
