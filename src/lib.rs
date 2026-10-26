//! Kasane - 階層型空間IDデータベース
//!
//! ## 実行モード
//! 本クレートは feature フラグにより 2 つの実行モードを提供します。
//!
//! - **サーバーモード** (`feature = "server"` / デフォルト):
//!   LMDB バックエンドと認証・ネットワーク gRPC サーバー機能を含むスタンドアロン実行構成。
//! - **組み込みモード** (`default-features = false`):
//!   インメモリバックエンド（[`repositories::memory::MemDb`]）を用い、
//!   プロセス内で直接 gRPC リクエストを処理する [`Embedded`] アダプタを提供する Wasm・埋め込み向け構成。

pub mod error;
pub mod grpc;
pub mod models;
pub mod repositories;
pub mod services;

#[cfg(feature = "server")]
pub mod logging;

#[cfg(not(feature = "server"))]
pub mod embedded;
#[cfg(not(feature = "server"))]
pub use embedded::Embedded;

pub use repositories::Db;

/// リクエスト間で共有する状態。
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}
