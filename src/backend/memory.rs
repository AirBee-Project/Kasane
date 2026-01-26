use super::{Backend, ReadTransaction, WriteTransaction};
use crate::{DbError, Result};
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

    async fn new(_path: &str) -> Result<Self> {
        Ok(Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    async fn begin_read(&self) -> Result<Self::ReadTx<'_>> {
        #[cfg(not(target_arch = "wasm32"))]
        let data = self.data.read().map_err(|_| DbError::LockPoisoned)?.clone();

        #[cfg(target_arch = "wasm32")]
        let data = self.data.borrow().clone();

        Ok(MemoryReadTx { data })
    }

    async fn begin_write(&self) -> Result<Self::WriteTx<'_>> {
        #[cfg(not(target_arch = "wasm32"))]
        let snapshot = self.data.read().map_err(|_| DbError::LockPoisoned)?.clone();

        #[cfg(target_arch = "wasm32")]
        let snapshot = self.data.borrow().clone();

        Ok(MemoryWriteTx {
            staging: snapshot,
            target: self.data.clone(),
        })
    }
}

// --- Read Transaction ---
pub struct MemoryReadTx {
    data: HashMap<Vec<u8>, Vec<u8>>,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ReadTransaction for MemoryReadTx {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.data.get(key).cloned())
    }

    async fn show_fields(&self) -> Result<Vec<String>> {
        todo!()
    }
}

// --- Write Transaction ---
pub struct MemoryWriteTx {
    staging: HashMap<Vec<u8>, Vec<u8>>,
    target: DbMap,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ReadTransaction for MemoryWriteTx {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.staging.get(key).cloned())
    }

    async fn show_fields(&self) -> Result<Vec<String>> {
        todo!()
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl WriteTransaction for MemoryWriteTx {
    async fn set(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        self.staging.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    // (追加予定のフィールド操作系メソッドは todo!() のままでOK)
    async fn create_field(&mut self, _field_name: String) -> Result<()> {
        todo!()
    }
    async fn drop_field(&mut self, _field_name: String) -> Result<()> {
        todo!()
    }
    async fn rename_field(
        &mut self,
        _old_field_name: String,
        _new_field_name: String,
    ) -> Result<()> {
        todo!()
    }

    async fn commit(self) -> Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        let mut guard = self.target.write().map_err(|_| DbError::LockPoisoned)?;

        #[cfg(target_arch = "wasm32")]
        let mut guard = self.target.borrow_mut();

        // ステージングの内容を本体に上書き（ここでのみ反映される）
        *guard = self.staging;
        Ok(())
    }

    // ★修正箇所★
    async fn rollback(self) -> Result<()> {
        // 何もしないで終了 -> self.staging が破棄される -> ロールバック完了
        Ok(())
    }
}
