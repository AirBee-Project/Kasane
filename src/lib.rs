pub mod backend;
use backend::Backend;
use kasane_logic::RangeId;

#[cfg(target_arch = "wasm32")]
type InnerBackend = backend::memory::MemoryBackend;

#[cfg(all(not(target_arch = "wasm32"), feature = "tikv"))]
type InnerBackend = backend::tikv::TikvBackend;

#[cfg(all(not(target_arch = "wasm32"), feature = "redb", not(feature = "tikv")))]
type InnerBackend = backend::redb::RedbBackend;

#[cfg(all(
    not(target_arch = "wasm32"),
    not(feature = "redb"),
    not(feature = "tikv")
))]
type InnerBackend = backend::memory::MemoryBackend;

pub struct KasaneDb {
    inner: InnerBackend,
}

impl KasaneDb {
    /// DBを開く
    pub async fn open(path_or_url: &str) -> anyhow::Result<Self> {
        let inner = InnerBackend::new(path_or_url).await?;
        Ok(Self { inner })
    }

    pub async fn begin_read(&self) -> impl backend::Transaction + '_ {
        self.inner.begin_read().await
    }

    pub async fn begin_write(&self) -> impl backend::Transaction + '_ {
        self.inner.begin_write().await
    }
}

// Wasm用エントリポイント
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}
