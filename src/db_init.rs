use redb::{Database, TableDefinition, TableHandle};

use crate::models::layer::LayerMetadata;

/// KasaneのLayer情報を管理するための内部テーブル
///
/// Key: Layerの名前
/// Value: Layerの名前以外の情報
pub const LAYERS: TableDefinition<&str, LayerMetadata> = TableDefinition::new("1");

/// システム内一意な番号を発行するためのテーブル
///
/// Key: 対象のエンティティ
/// Value: 次のID(u64)
pub const LAYER_IDS: TableDefinition<&str, u64> = TableDefinition::new("2");

/// LayerのIDを管理するためのKey
pub const LAYER_IDS_KEY: &str = "t";

/// 空間IDと値の対応を管理するためのテーブル
///
/// Key: (LayerのID, 空間IDのエンコードバイト)
/// Value: 値のバイト列
pub const SPATIAL_IDS: TableDefinition<(u64, [u8; 12]), &[u8]> = TableDefinition::new("3");

/// 空間IDに対して割り当てられた値を管理するためのテーブル
/// ID > Valueの紐づけを行う。
///
/// Key:(LayerのID,ValueのID)
/// Value: Valueのバイト列
pub const ID_TO_VALUE: TableDefinition<(u64, u64), &[u8]> = TableDefinition::new("4");

//// 空間IDに対して割り当てられた値を管理するためのテーブル
/// Value > IDの紐づけを行う。
///
/// Key:(LayerのID,Valueのバイト列)
/// Value: VlaueのID
pub const VALUE_TO_ID: TableDefinition<(u64, &[u8]), u64> = TableDefinition::new("5");

pub fn initialize_database(path: &str) -> Database {
    let database = Database::create(path).unwrap();

    let write_txn = database.begin_write().unwrap();

    {
        let _ = write_txn.open_table(LAYERS).unwrap();
        let mut ids = write_txn.open_table(LAYER_IDS).unwrap();
        let _ = ids.insert(LAYER_IDS_KEY, 0u64).unwrap();
    }

    write_txn.commit().unwrap();

    database
}
