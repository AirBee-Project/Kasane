//! Kasane - 階層型空間IDデータベース
//!
//! 入口は [`Kasane`] だけで、feature によって中のストレージと使えるメソッドが変わる。
//!
//! - `server`（既定）: LMDB に保存する。`Kasane::open` で開き、`Kasane::bind` で
//!   gRPC サーバーとして待ち受ける。
//! - `default-features = false`: インメモリに保存する。`Kasane::new` で作り、
//!   `Kasane::handle` でプロセス内から gRPC リクエストを処理する（Wasm・組み込み向け）。
//!   利用者は root だけで、AuthService と UserService は持たない。

pub mod error;
pub mod grpc;
#[doc(hidden)]
pub mod models;
#[doc(hidden)]
pub mod repositories;
#[doc(hidden)]
pub mod services;

#[cfg(feature = "server")]
pub mod logging;

#[cfg(not(feature = "server"))]
mod embedded;

#[cfg(feature = "server")]
pub use grpc::BoundServer;

use repositories::Db;

/// Kasane のインスタンス。
///
/// server では複製してもストレージを共有するハンドルになる（tonic がサービスごとに複製するため）。
/// 組み込みでは複製できないので、`Kasane::handle` の `&mut self` で呼び出しが排他になる。
#[cfg_attr(feature = "server", derive(Clone))]
pub struct Kasane {
    pub(crate) db: Db,
}

impl Kasane {
    /// ストレージを直接読み書きするテストのための入口。
    #[doc(hidden)]
    pub fn storage(&self) -> &Db {
        &self.db
    }
}

#[cfg(feature = "server")]
impl Kasane {
    /// `path` の LMDB を開く。無ければ作り、root を用意する。
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, error::AppError> {
        Ok(Self {
            db: repositories::lmdb::initialize_database(path)?,
        })
    }
}
