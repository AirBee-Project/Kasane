use std::{io::Read, path::Path};

use redb::{Database, ReadableDatabase, TableDefinition};

use crate::{error::Error, write::WriteTx};
pub mod error;
pub mod write;

///field_nameとfiled_idの変換
pub const FILED_DICTIONARY: TableDefinition<&str, u64> = TableDefinition::new("filed_dictonary");

///全体の管理に必要な情報を入れておく
pub const GLOBAL_STATE: TableDefinition<&str, u64> = TableDefinition::new("global_state");
const FIELD_ID_KEY: &str = "next_field_id";

///これは組み込みのデータベースである
/// redbをバックエンドとして動作する
/// プリミティブな動作を提供する
/// リッチな動作（最適な実行手法など）は求めない
/// redbとの境界を満たし、トランザクション機能を提供することを主眼とする
/// ログ機能、型安全性なども提供しない
/// 型は全て&[u8]である
pub struct Kasane {
    db: Database,
}

pub struct ReadTx {
    tx: redb::ReadTransaction,
}

impl Kasane {
    ///データベースの初期化
    pub fn init(path: &Path) -> Result<Self, Error> {
        let db = Database::create(path)?;
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(FILED_DICTIONARY);
        }
        write_txn.commit()?;

        Ok(Self { db })
    }

    ///read transactionの発行
    pub fn read_tx(&self) -> Result<ReadTx, Error> {
        let tx = self.db.begin_read()?;
        Ok(ReadTx { tx })
    }

    ///write transactionの発行
    pub fn write_tx(&self) -> Result<WriteTx, Error> {
        let tx = self.db.begin_write()?;
        Ok(WriteTx { tx })
    }
}
