use redb::{Database, TableDefinition};

use crate::models::table::TableMetadata;

/// KasaneのTable情報を管理するための内部Table
///
/// Key: Tableの名前
/// Value: Tableの名前以外の情報
pub const TABLES: TableDefinition<&str, TableMetadata> = TableDefinition::new("1");

/// システム内一意な番号を発行するためのTABLE
///
/// Key: 対象のエンティティ
/// Value: 次のID(u64)
pub const TABLE_IDS: TableDefinition<&str, u64> = TableDefinition::new("2");

///TableのIDを管理するためのKey
pub const TABLE_IDS_KEY: &str = "t";

/// 空間IDと値の対応を管理するためのTABLE
///
/// Key: (TableのID, 12バイトのID)
/// Value: 値のバイト列
pub const SPAITAL_IDS: TableDefinition<(u64, [u8; 12]), &[u8]> = TableDefinition::new("3");

pub fn initialize_database(path: &str) -> Database {
    let database = Database::create(path).unwrap();

    let write_txn = database.begin_write().unwrap();

    {
        let _ = write_txn.open_table(TABLES).unwrap();
        let mut ids = write_txn.open_table(TABLE_IDS).unwrap();
        let _ = ids.insert(TABLE_IDS_KEY, 0u64).unwrap();
    }

    write_txn.commit().unwrap();

    database
}
