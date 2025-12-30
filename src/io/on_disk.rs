use crate::{
    error::Error,
    io::{models::FieldDef, Kasane},
    transaction::{read::ReadTxTrait, write::WriteTxTrait},
};

use redb::{
    Database, ReadTransaction, ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction,
};
use std::path::Path;

/* =========================
   OnDisk Database
========================= */

pub struct OnDisk {
    pub(crate) db: Database,
}

/* =========================
   Transactions (User-facing)
========================= */

pub struct OnDiskWriteTx {
    pub(crate) inner: WriteTransaction,
}

pub struct OnDiskReadTx {
    pub(crate) inner: ReadTransaction,
}

/* =========================
   Table Definitions
========================= */

// フィールド一覧
pub(crate) static FIELD_TABLE: TableDefinition<String, FieldDef> =
    TableDefinition::new("FIELD_TABLE");

// メタ情報
pub(crate) static META_TABLE: TableDefinition<&'static str, u64> =
    TableDefinition::new("META_TABLE");

// META_TABLE keys
pub const META_FIELD_ID: &str = "FieldID";

impl Kasane for OnDisk {
    /// DB を開く（存在しなければ作成）＋スキーマ初期化
    fn new(path: &Path) -> Result<Self, Error> {
        let db = if path.exists() {
            Database::open(path)?
        } else {
            Database::create(path)?
        };

        //トランザクションを開始
        let write_tx = db.begin_write()?;

        // スキーマ初期化
        {
            // テーブル作成（存在しなければ）
            write_tx.open_table(FIELD_TABLE)?;
            let mut meta_table = write_tx.open_table(META_TABLE)?;

            // FieldID 初期化（次に割り当てる ID）
            if meta_table.get(META_FIELD_ID)?.is_none() {
                meta_table.insert(META_FIELD_ID, 0)?;
            }
        }

        //トランザクションを反映
        write_tx.commit()?;

        Ok(Self { db })
    }

    fn write_begin(&mut self) -> Result<impl WriteTxTrait, Error> {
        let tx = self.db.begin_write()?;
        Ok(OnDiskWriteTx { inner: tx })
    }

    fn read_begin(&self) -> Result<impl ReadTxTrait, Error> {
        let tx = self.db.begin_read()?;
        Ok(OnDiskReadTx { inner: tx })
    }
}
