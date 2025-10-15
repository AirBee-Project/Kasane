use crate::UserError;
use sled::Db;
use std::{env, path::PathBuf};
pub mod create_key;
pub mod create_space;
pub mod create_user;
pub mod info_key;
pub mod show_keys;
pub mod show_spaces;
pub mod tools;
pub mod verify_user;
pub mod version;
pub struct Storage {
    pub db: Db,
}

impl Storage {
    pub fn new(path: Option<PathBuf>) -> Result<Self, UserError> {
        // sledデータベースを開く（ディレクトリがなければ作成）
        let db_path = path.unwrap_or(env::current_dir().unwrap().join("sled_db"));
        let db = sled::open(&db_path)?;
        Ok(Self { db })
    }
}
