use redb::{Database, TableDefinition};
use std::{env, path::PathBuf};
use uuid::Uuid;

use crate::{r#type::uuid::UuidKey, user_error::UserError};
pub mod cleanup_expired_sessions;
pub mod count_keepalive_sessions;
pub mod create_key;
pub mod create_session;
pub mod create_space;
pub mod create_user;
pub mod drop_session;
pub mod info_key;
pub mod info_space;
pub mod insert_value;
pub mod show_keys;
pub mod show_spaces;
pub mod tools;
pub mod validate_session;
pub mod verify_user;
pub mod version;

//Tableの定義
pub const USER_TABLE: TableDefinition<&str, UuidKey> = TableDefinition::new("user_table");
pub const USER_PASSWORD: TableDefinition<UuidKey, &str> = TableDefinition::new("user_password");

pub struct Storage {
    pub db: Database,
}

impl Storage {
    ///ストレージを新しく作成
    pub fn new(path: Option<PathBuf>) -> Result<Self, UserError> {
        let db_path = path.unwrap_or(env::current_dir().unwrap().join("default"));
        let db = Database::create(db_path)?;
        Ok(Storage { db })
    }
}
