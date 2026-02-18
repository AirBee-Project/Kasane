use std::path::Path;

use redb::{Database, ReadableDatabase};

use crate::{error::Error, read::ReadTx, write::WriteTx};
pub mod error;
pub mod read;
pub mod scanner;
pub mod tables;
pub mod write;

pub struct Kasane {
    db: Database,
}

impl Kasane {
    ///データベースの初期化
    pub fn init(path: &Path) -> Result<Self, Error> {
        let db = Database::create(path)?;
        let write_txn = db.begin_write()?;
        {
            //全てのTableを開いてエラーがでないかを検証
            let _ = write_txn.open_table(Self::FILED)?;
            let _ = write_txn.open_table(Self::GLOBAL_STATE)?;
            let _ = write_txn.open_table(Self::F)?;
            let _ = write_txn.open_table(Self::X)?;
            let _ = write_txn.open_table(Self::Y)?;
            let _ = write_txn.open_table(Self::MAIN)?;
            let _ = write_txn.open_table(Self::DICTIONARY)?;
            let _ = write_txn.open_table(Self::VALUE_REVERSE)?;
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
