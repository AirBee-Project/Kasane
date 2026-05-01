use redb::{Database, TableDefinition};

/// KasaneのTable情報を管理するための内部Table
///
/// Key: Tableの名前
/// Value: Tableの情報をシリアライズしたバイト列
pub const TABLES: TableDefinition<&str, &[u8]> = TableDefinition::new("table_info");

pub fn initialize_database(path: &str) -> Database {
    let database = Database::create(path).unwrap();

    let write_txn = database.begin_write().unwrap();

    {
        let _ = write_txn.open_table(TABLES).unwrap();
    }

    write_txn.commit().unwrap();

    database
}
