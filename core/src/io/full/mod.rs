use redb::{Database, DatabaseError, MultimapTableDefinition, TableDefinition, Value};
use std::{env, path::PathBuf};

use crate::{
    io::full::kv_type::{
        key_table_key::KeyTableKey, user_session_key::UserSessionKey, uuid::UuidKey,
    },
    json::input::{DatabaseCommand, KeyCommand, SpaceCommand},
    user_error::UserError,
};

pub mod space;
pub mod key;
pub mod value;
pub mod user;
pub mod session;
pub mod kv_type;
pub mod tools;

// Tableの定義

// ユーザー関連
pub const USER_TABLE: TableDefinition<&str, UuidKey> = TableDefinition::new("user_table");
pub const USER_PASSWORD: TableDefinition<UuidKey, &str> = TableDefinition::new("user_password");
pub const USER_SESSION: TableDefinition<UserSessionKey, UuidKey> =
    TableDefinition::new("user_session");

//権限関連
pub const PERMISSION_DATABASE: MultimapTableDefinition<UuidKey, DatabaseCommand> =
    MultimapTableDefinition::new("permission_database");
pub const PERMISSION_SPACE: MultimapTableDefinition<UuidKey, SpaceCommand> =
    MultimapTableDefinition::new("permission_space");
pub const PERMISSION_KEY: MultimapTableDefinition<UuidKey, KeyCommand> =
    MultimapTableDefinition::new("permission_key");

// 本体機能
pub const SPACE_TABLE: TableDefinition<&str, UuidKey> = TableDefinition::new("space_table");
pub const KEY_TABLE: TableDefinition<KeyTableKey, UuidKey> = TableDefinition::new("key_table");

pub struct Storage {
    pub db: Database,
}

impl Storage {
    /// ストレージを作成または既存ファイルを読み込み
    pub fn new(path: PathBuf) -> Result<Self, UserError> {
        // Database::create は存在すれば読み込み、なければ新規作成
        let db = Database::create(&path).unwrap();

        Ok(Storage { db })
    }
}
