use crate::error::AppError;

pub type Db = crate::repositories::lmdb::AppDb;

pub const NAME: &str = "lmdb";

pub fn default_target() -> String {
    std::env::var("DATABASE_DIR").unwrap_or_else(|_| "default_kasane_db".to_string())
}

pub async fn open(target: &str) -> Result<Db, AppError> {
    crate::repositories::lmdb::initialize_database(target)
}
