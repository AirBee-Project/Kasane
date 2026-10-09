//! TiKV・LMDB・メモリのバックエンドを[Db]として抽象化する。
//! service layerより上層ではバックエンドの差を意識する必要がない。

#[cfg(any(
    all(feature = "backend-lmdb", feature = "backend-tikv"),
    all(feature = "backend-lmdb", feature = "backend-memory"),
    all(feature = "backend-tikv", feature = "backend-memory"),
))]
compile_error!(
    "Features `backend-lmdb`, `backend-tikv` and `backend-memory` are mutually exclusive; choose only one."
);

#[cfg(not(any(
    feature = "backend-lmdb",
    feature = "backend-tikv",
    feature = "backend-memory"
)))]
compile_error!(
    "No backend selected; enable one of `backend-lmdb`, `backend-tikv` or `backend-memory`."
);
use crate::error::AppError;

#[cfg(feature = "backend-lmdb")]
pub type Db = crate::repositories::lmdb::AppDb;

#[cfg(feature = "backend-tikv")]
pub type Db = crate::repositories::tikv::TikvDb;

#[cfg(feature = "backend-memory")]
pub type Db = crate::repositories::memory::MemoryDb;

#[cfg(feature = "backend-lmdb")]
pub const NAME: &str = "lmdb";

#[cfg(feature = "backend-tikv")]
pub const NAME: &str = "tikv";

#[cfg(feature = "backend-memory")]
pub const NAME: &str = "memory";

#[cfg(feature = "backend-lmdb")]
pub fn default_target() -> String {
    std::env::var("DATABASE_DIR").unwrap_or_else(|_| "default_kasane_db".to_string())
}

#[cfg(feature = "backend-tikv")]
pub fn default_target() -> String {
    crate::repositories::tikv::TikvConfig::from_env()
        .pd_endpoints
        .join(",")
}

#[cfg(feature = "backend-memory")]
pub fn default_target() -> String {
    "memory".to_string()
}

#[cfg(feature = "backend-lmdb")]
pub async fn open(target: &str) -> Result<Db, AppError> {
    crate::repositories::lmdb::initialize_database(target)
}

#[cfg(feature = "backend-tikv")]
pub async fn open(target: &str) -> Result<Db, AppError> {
    use crate::repositories::tikv::{TikvConfig, TikvDb};
    let db = TikvDb::connect(TikvConfig::from_endpoints(target)).await?;
    let gc_config = crate::repositories::tikv::GcConfig::from_env();
    db.spawn_sweeper(gc_config.clone());
    db.spawn_gc(gc_config);
    Ok(db)
}

/// `target` は使わない。開くたびに root だけを持つ空のストレージになる。
#[cfg(feature = "backend-memory")]
pub async fn open(_target: &str) -> Result<Db, AppError> {
    crate::repositories::memory::MemoryDb::new()
}
