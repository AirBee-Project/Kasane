use bincode::{Decode, Encode};
use redb::{Database, MultimapTableDefinition, TableDefinition, TypeName, Value};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    env,
    path::PathBuf,
};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceKeyTableValue(pub HashSet<UuidKey>);

impl Value for SpaceKeyTableValue {
    type SelfType<'a> = SpaceKeyTableValue;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let mut set = HashSet::new();
        for chunk in data.chunks_exact(16) {
            let uuid = Uuid::from_slice(chunk).expect("invalid uuid bytes");
            set.insert(UuidKey(uuid));
        }
        SpaceKeyTableValue(set)
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut v = Vec::with_capacity(value.0.len() * 16);
        for UuidKey(uuid) in &value.0 {
            v.extend_from_slice(uuid.as_bytes());
        }
        v
    }

    fn type_name() -> TypeName {
        TypeName::new("SpaceKeyTableValue")
    }
}

//Tableの定義
pub const USER_TABLE: TableDefinition<&str, UuidKey> = TableDefinition::new("user_table");
pub const USER_PASSWORD: TableDefinition<UuidKey, &str> = TableDefinition::new("user_password");
pub const SPACE_TABLE: TableDefinition<&str, UuidKey> = TableDefinition::new("space_table");
pub const SPACE_KEY_TABLE: TableDefinition<UuidKey, SpaceKeyTableValue> =
    TableDefinition::new("space_key_table");

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
