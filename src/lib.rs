pub mod error;
pub mod grpc;
#[cfg(feature = "server")]
pub mod logging;
pub mod models;
#[doc(hidden)]
pub mod repositories;
#[doc(hidden)]
pub mod services;

use repositories::Db;

/// Kasane のインスタンス。データベースの操作はメソッドとして持ち、通信路は持たない。
pub struct Kasane {
    pub(crate) db: Db,
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

#[cfg(not(feature = "server"))]
impl Kasane {
    /// 空のインメモリのインスタンスを作る。インスタンスごとに独立したデータを持つ。
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(not(feature = "server"))]
impl Default for Kasane {
    fn default() -> Self {
        Self {
            db: repositories::memory::Memory::new(),
        }
    }
}
