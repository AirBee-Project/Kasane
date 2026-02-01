use super::{Backend, ReadTransaction, WriteTransaction};
use crate::Error;
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

#[derive(Clone)]
pub struct MemoryBackend {
    data: DbMap,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl Backend for MemoryBackend {
    type ReadTx<'a> = MemoryReadTx;
    type WriteTx<'a> = MemoryWriteTx;

    fn new(_path: &str) -> Result<Self, Error> {
        Ok(Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn begin_read(&self) -> Result<Self::ReadTx<'_>, Error> {
        #[cfg(not(target_arch = "wasm32"))]
        let data = self.data.read().map_err(|_| Error::LockPoisoned)?.clone();

        #[cfg(target_arch = "wasm32")]
        let data = self.data.borrow().clone();

        Ok(MemoryReadTx { data })
    }

    fn begin_write(&self) -> Result<Self::WriteTx<'_>, Error> {
        #[cfg(not(target_arch = "wasm32"))]
        let snapshot = self.data.read().map_err(|_| Error::LockPoisoned)?.clone();

        #[cfg(target_arch = "wasm32")]
        let snapshot = self.data.borrow().clone();

        Ok(MemoryWriteTx {
            staging: snapshot,
            target: self.data.clone(),
        })
    }
}

pub struct MemoryReadTx {
    data: HashMap<Vec<u8>, Vec<u8>>,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ReadTransaction for MemoryReadTx {
    fn show_field_ids(&self) -> Result<Vec<super::FieldId>, Error> {
        todo!()
    }

    fn select(
        &self,
        field_id: super::FieldId,
        range: kasane_logic::SetOnMemory,
    ) -> kasane_logic::TableOnMemory<&[u8]> {
        todo!()
    }
}

pub struct MemoryWriteTx {
    staging: HashMap<Vec<u8>, Vec<u8>>,
    target: DbMap,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ReadTransaction for MemoryWriteTx {
    fn show_field_ids(&self) -> Result<Vec<super::FieldId>, Error> {
        todo!()
    }

    fn select(
        &self,
        field_id: super::FieldId,
        range: kasane_logic::SetOnMemory,
    ) -> kasane_logic::TableOnMemory<&[u8]> {
        todo!()
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl WriteTransaction for MemoryWriteTx {
    fn create_field(&mut self) -> Result<super::FieldId, Error> {
        todo!()
    }

    fn drop_field(&mut self, field_id: super::FieldId) -> Result<(), Error> {
        todo!()
    }

    fn insert(
        &mut self,
        field_id: super::FieldId,
        range: kasane_logic::SetOnMemory,
        value: &[u8],
    ) -> Result<(), Error> {
        todo!()
    }

    fn merge(
        &mut self,
        field_id: super::FieldId,
        range: kasane_logic::SetOnMemory,
        value: &[u8],
    ) -> Result<(), Error> {
        todo!()
    }

    fn update(
        &mut self,
        field_id: super::FieldId,
        range: kasane_logic::SetOnMemory,
        value: &[u8],
    ) -> Result<(), Error> {
        todo!()
    }

    fn commit(self) -> Result<(), Error>
    where
        Self: Sized,
    {
        todo!()
    }
}
