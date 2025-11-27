use redb::{Database, MultimapTableDefinition, TableDefinition};
use std::path::PathBuf;

use crate::{
    io::full::table_types::{
        dimension_key::DimensionKey, dimension_value::DimensionValue,
        key_table_key::KeyTableKey, reverse_key::ReverseKey, reverse_value::ReverseValue,
        user_session_key::UserSessionKey, uuid::UuidKey, value_entry::ValueEntry,
        value_reverse_key::ValueReverseKey,
    },
    user_error::UserError,
};

/// Command extension implementations (impl blocks for input types)
pub mod command_impls;
pub mod key;
pub mod session;
pub mod space;
/// Types used as keys/values in redb table definitions
pub mod table_types;
pub mod user;
pub mod value;

// ユーザー関連
pub const USER_TABLE: TableDefinition<&str, UuidKey> = TableDefinition::new("user_table");
pub const USER_PASSWORD: TableDefinition<UuidKey, &str> = TableDefinition::new("user_password");
pub const USER_SESSION: TableDefinition<UserSessionKey, UuidKey> =
    TableDefinition::new("user_session");

// 本体機能
pub const SPACE_TABLE: TableDefinition<&str, UuidKey> = TableDefinition::new("space_table");
pub const KEY_TABLE: TableDefinition<KeyTableKey, UuidKey> = TableDefinition::new("key_table");

//各次元のBitVec
pub const F_TABLE: TableDefinition<DimensionKey, DimensionValue> = TableDefinition::new("f_table");
pub const X_TABLE: TableDefinition<DimensionKey, DimensionValue> = TableDefinition::new("x_table");
pub const Y_TABLE: TableDefinition<DimensionKey, DimensionValue> = TableDefinition::new("y_table");

//時空間IDの逆引き用のTable
pub const REVERSE_TABLE: TableDefinition<ReverseKey, ReverseValue> =
    TableDefinition::new("reverse_table");

//Value管理用のTable
pub const VALUE_TABLE: TableDefinition<UuidKey, ValueEntry> = TableDefinition::new("value_table");
pub const VALUE_REVERSE_TABLE: MultimapTableDefinition<ValueReverseKey, u64> =
    MultimapTableDefinition::new("value_table");

pub struct Storage {
    pub db: Database,
}

impl Storage {
    /// ストレージを作成または既存ファイルを読み込み
    pub fn new(path: PathBuf) -> Result<Self, UserError> {
        // Database::create は存在すれば読み込み、なければ新規作成
        let db = Database::create(&path)?;
        Ok(Storage { db })
    }
}
