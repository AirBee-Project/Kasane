//! TiKVとLMDBのバックエンドを[Db]として抽象化する。
//! service layerより上層ではバックエンドの差を意識する必要がない。

#[cfg(all(feature = "backend-lmdb", feature = "backend-tikv"))]
compile_error!(
    "Features `backend-lmdb` and `backend-tikv` are mutually exclusive; choose only one."
);

#[cfg(not(any(feature = "backend-lmdb", feature = "backend-tikv")))]
compile_error!("No backend selected; enable either `backend-lmdb` or `backend-tikv`.");
use crate::error::AppError;

#[cfg(feature = "backend-lmdb")]
pub type Db = crate::repositories::lmdb::AppDb;

#[cfg(feature = "backend-tikv")]
pub type Db = crate::repositories::tikv::TikvDb;

#[cfg(feature = "backend-lmdb")]
pub const NAME: &str = "lmdb";

#[cfg(feature = "backend-tikv")]
pub const NAME: &str = "tikv";

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
