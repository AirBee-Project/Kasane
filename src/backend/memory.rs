use super::{Backend, Transaction};
use async_trait::async_trait;
use std::collections::HashMap;

// --- 環境別の型エイリアス ---
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
// -------------------------

#[derive(Clone)]
pub struct MemoryBackend {
    data: DbMap,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Backend for MemoryBackend {
    type Tx<'a> = MemoryTx;

    async fn new(_path: &str) -> anyhow::Result<Self> {
        Ok(Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    async fn begin_read(&self) -> Self::Tx<'_> {
        MemoryTx {
            data: self.data.clone(),
        }
    }

    async fn begin_write(&self) -> Self::Tx<'_> {
        MemoryTx {
            data: self.data.clone(),
        }
    }
}

pub struct MemoryTx {
    data: DbMap,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<'a> Transaction for MemoryTx {
    async fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        // Native: read().unwrap(), Wasm: borrow()
        #[cfg(not(target_arch = "wasm32"))]
        let guard = self
            .data
            .read()
            .map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
        #[cfg(target_arch = "wasm32")]
        let guard = self.data.borrow();

        Ok(guard.get(key).cloned())
    }

    async fn set(&mut self, key: &[u8], value: &[u8]) -> anyhow::Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        let mut guard = self
            .data
            .write()
            .map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
        #[cfg(target_arch = "wasm32")]
        let mut guard = self.data.borrow_mut();

        guard.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    async fn commit(self) -> anyhow::Result<()> {
        Ok(()) // メモリ版は即時反映しているので何もしない
    }
}
