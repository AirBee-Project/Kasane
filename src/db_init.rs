use redb::{Database, TableDefinition};

use crate::models::layer::LayerMetadata;

/// KasaneのLayer情報を管理するための内部テーブル
///
/// Key: Layerの名前
/// Value: Layerの名前以外の情報
pub const LAYERS: TableDefinition<&str, LayerMetadata> = TableDefinition::new("1");

/// LayerのID衝突チェック用インデックス
///
/// Key: LayerのID
/// Value: 空
pub const LAYER_ID_INDEX: TableDefinition<[u8; 16], ()> = TableDefinition::new("2");

/// 空間IDと値の対応を管理するためのテーブル
///
/// Key: (LayerのID, 空間IDのエンコードバイト)
/// Value: 値のバイト列
pub const SPATIALID_TO_VALUE: TableDefinition<([u8; 16], [u8; 12]), &[u8]> =
    TableDefinition::new("3");

/// 値と空間IDに対応を管理するためのテーブル
///
/// Key:(LayerのID,値のバイト列、空間IDのエンコードバイト列)
/// Value: 空
pub const VALUE_TO_SPATIALID: TableDefinition<([u8; 16], &[u8], [u8; 12]), ()> =
    TableDefinition::new("4");

pub fn initialize_database(path: &str) -> Database {
    let database = Database::create(path).unwrap();

    let write_txn = database.begin_write().unwrap();

    {
        let _ = write_txn.open_table(LAYERS).unwrap();
        let _ = write_txn.open_table(LAYER_ID_INDEX).unwrap();
    }

    write_txn.commit().unwrap();

    database
}
