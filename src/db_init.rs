use redb::{Database, ReadableTableMetadata, TableDefinition};

use crate::models::users::UserMetadata;
use crate::models::{database::DatabaseMetadata, database::table::TableMetadata};
use crate::services::auth::hash_password;
use uuid::Uuid;

pub const DATABASES: TableDefinition<&str, DatabaseMetadata> = TableDefinition::new("0");

pub const TABLES: TableDefinition<([u8; 16], &str), TableMetadata> = TableDefinition::new("1");

/// TableのID衝突チェック用インデックス
///
/// Key: TableのID
/// Value: 空
pub const TABLE_ID_INDEX: TableDefinition<[u8; 16], ()> = TableDefinition::new("2");

/// 空間IDと値の対応を管理するためのテーブル
///
/// Key: (TableのID, 空間IDのエンコードバイト)
/// Value: 値のバイト列
pub const SPATIALID_TO_VALUE: TableDefinition<([u8; 16], [u8; 12]), &[u8]> =
    TableDefinition::new("3");

/// 値と空間IDに対応を管理するためのテーブル
///
/// Key:(TableのID,値のバイト列、空間IDのエンコードバイト列)
/// Value: 空
#[allow(clippy::type_complexity)]
pub const VALUE_TO_SPATIALID: TableDefinition<([u8; 16], &[u8], [u8; 12]), ()> =
    TableDefinition::new("4");

pub const USERS: TableDefinition<&str, &str> = TableDefinition::new("users");
pub const USER_PRIVILEGES: TableDefinition<([u8; 16], [u8; 16]), u8> =
    TableDefinition::new("user_privileges");

#[tracing::instrument]
pub fn initialize_database(path: &str) -> Database {
    tracing::info!("Initializing database at: {}", path);
    let database = Database::create(path).unwrap_or_else(|e| {
        tracing::error!("Failed to create database at {}: {}", path, e);
        panic!("Failed to create database: {}", e);
    });

    let write_txn = database.begin_write().unwrap();

    {
        tracing::debug!("Opening internal tables...");
        let _ = write_txn.open_table(DATABASES).unwrap();
        let _ = write_txn.open_table(TABLES).unwrap();
        let _ = write_txn.open_table(TABLE_ID_INDEX).unwrap();
        let _ = write_txn.open_table(SPATIALID_TO_VALUE).unwrap();
        let _ = write_txn.open_table(VALUE_TO_SPATIALID).unwrap();

        let mut users_table = write_txn.open_table(USERS).unwrap();
        let _ = write_txn.open_table(USER_PRIVILEGES).unwrap();

        if users_table.is_empty().unwrap() {
            let default_username =
                std::env::var("KASANE_ROOT_USERNAME").unwrap_or_else(|_| "root".to_string());
            let default_password =
                std::env::var("KASANE_ROOT_PASSWORD").unwrap_or_else(|_| "password".to_string());

            tracing::info!("Creating default root user: {}", default_username);
            let hash = hash_password(&default_password).unwrap();
            let root_meta = UserMetadata {
                id: Uuid::now_v7(),
                password_hash: hash,
                is_global_admin: true,
            };
            let json = serde_json::to_string(&root_meta).unwrap();
            users_table
                .insert(default_username.as_str(), json.as_str())
                .unwrap();
        }
    }

    write_txn.commit().unwrap();
    tracing::info!("Database initialized successfully");

    database
}
