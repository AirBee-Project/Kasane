pub mod backend;
use backend::{Backend, ReadTransaction, WriteTransaction};

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
    /// - Redb: ファイルパス (例: "./data.redb")
    /// - TiKV: PDアドレス (例: "127.0.0.1:2379")
    /// - Memory: 無視されます
    pub async fn open(connection_string: &str) -> anyhow::Result<Self> {
        let inner = InnerBackend::new(connection_string).await?;
        Ok(Self { inner })
    }

    /// 読み取り専用トランザクションを開始する
    ///
    /// 戻り値は `impl ReadTransaction` なので、バックエンドの具体的な型を意識せずに使える。
    /// ライフタイム `'_` は、トランザクションがDB本体(`&self`)より長く生きられないことを保証する。
    pub async fn begin_read(&self) -> anyhow::Result<impl ReadTransaction + '_> {
        self.inner.begin_read().await
    }

    /// 書き込みトランザクションを開始する
    ///
    /// `WriteTransaction` は `ReadTransaction` を継承しているため、
    /// 書き込み中に `.get()` で自分の変更を読むことも可能。
    pub async fn begin_write(&self) -> anyhow::Result<impl WriteTransaction + '_> {
        self.inner.begin_write().await
    }
}

// --- Wasm初期化用 ---
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}
