pub mod backend;
pub mod error;

use backend::{Backend, ReadTransaction, WriteTransaction};
pub use error::Error;

/// このクレート全体で使うResult型
pub type Result<T> = std::result::Result<T, Error>;

// --- バックエンドの実体決定ロジック ---

// 1. Wasm: 強制的にMemory
#[cfg(target_arch = "wasm32")]
type InnerBackend = backend::memory::MemoryBackend;

// 2. Native & TiKV有効
#[cfg(all(not(target_arch = "wasm32"), feature = "tikv"))]
type InnerBackend = backend::tikv::TikvBackend;

// 3. Native & Redb有効 (TiKVが無効)
#[cfg(all(not(target_arch = "wasm32"), feature = "redb", not(feature = "tikv")))]
type InnerBackend = backend::redb::RedbBackend;

// 4. Native & Memory有効 (Redb/TiKV無効)
#[cfg(all(
    not(target_arch = "wasm32"),
    not(feature = "redb"),
    not(feature = "tikv")
))]
type InnerBackend = backend::memory::MemoryBackend;

// ----------------------------------------

pub struct KasaneDb {
    inner: InnerBackend,
}

impl KasaneDb {
    /// DBを開く
    pub async fn open(connection_string: &str) -> Result<Self> {
        let inner = InnerBackend::new(connection_string);
        Ok(Self { inner })
    }

    pub async fn begin_read(&self) -> Result<impl ReadTransaction + '_> {
        self.inner.begin_read()
    }

    pub async fn begin_write(&self) -> Result<impl WriteTransaction + '_> {
        self.inner.begin_write()
    }
}

// Wasm初期化用
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}
