use kasane_logic::SingleId;
use redb::{Database, TableDefinition};

use crate::models::table::entity::TableMetadata;

/// KasaneのTable情報を管理するための内部Table
///
/// Key: Tableの名前
/// Value: Tableの名前以外の情報
pub const TABLES: TableDefinition<&str, TableMetadata> = TableDefinition::new("1");

/// システム内一意な番号を発行するためのTABLE
///
/// Key: 対象のエンティティ
/// Value: 次のRank(u64)
pub const RANKS: TableDefinition<&str, u64> = TableDefinition::new("2");

///TableのRANKを管理するためのKey
pub const RANKS_KEY_TABLE: &str = "t";

/// 空間IDと値の対応を管理するためのTABLE
///
/// Key: (TableのRank, SingleIdのバイト列)
/// Value: 値のバイト列
pub const SPAITAL_IDS: TableDefinition<(u64, [u8; 12]), &[u8]> = TableDefinition::new("3");

pub fn initialize_database(path: &str) -> Database {
    let database = Database::create(path).unwrap();

    let write_txn = database.begin_write().unwrap();

    {
        let _ = write_txn.open_table(TABLES).unwrap();
        let mut ranks = write_txn.open_table(RANKS).unwrap();
        let _ = ranks.insert(RANKS_KEY_TABLE, 0u64).unwrap();
    }

    write_txn.commit().unwrap();

    database
}
