use crate::{
    error::Error,
    io::{FieldId, FlexRank, Kasane},
};

use kasane_logic::{segment::Segment, FlexId, RoaringTreemap};
use redb::{
    Database, ReadTransaction, ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction,
};
use std::path::Path;

pub struct OnDisk {
    pub(crate) db: Database,
}

pub struct OnDiskWriteTx {
    pub(crate) inner: WriteTransaction,
}

pub struct OnDiskReadTx {
    pub(crate) inner: ReadTransaction,
}

// フィールド一覧
pub(crate) static FIELD_TABLE: TableDefinition<String, FieldId> =
    TableDefinition::new("FIELD_TABLE");

// メタ情報
pub(crate) static META_TABLE: TableDefinition<&'static str, u64> =
    TableDefinition::new("META_TABLE");

//検索用の各次元のセグメント情報
pub(crate) static F: TableDefinition<(FieldId, [u8; Segment::ARRAY_LENGTH]), RoaringTreemap> =
    TableDefinition::new("F_SEGMENT_TABLE");
pub(crate) static X: TableDefinition<(FieldId, [u8; Segment::ARRAY_LENGTH]), RoaringTreemap> =
    TableDefinition::new("X_SEGMENT_TABLE");
pub(crate) static Y: TableDefinition<(FieldId, [u8; Segment::ARRAY_LENGTH]), RoaringTreemap> =
    TableDefinition::new("Y_SEGMENT_TABLE");

pub(crate) static MAIN: TableDefinition<
    (FieldId, u64),
    (
        [u8; Segment::ARRAY_LENGTH],
        [u8; Segment::ARRAY_LENGTH],
        [u8; Segment::ARRAY_LENGTH],
    ),
> = TableDefinition::new("ENCODE_ID_TABLE");

//Valueに対してクエリをかけられるようにするためのTable
pub(crate) static FORWARD: TableDefinition<(FieldId, Vec<u8>), ValueInfoDef> =
    TableDefinition::new("ENCODE_ID_TABLE");

// META_TABLE keys
pub const META_FIELD_ID: &str = "FieldId";

impl Kasane for OnDisk {
    type WriteTx = OnDiskWriteTx;
    type ReadTx = OnDiskReadTx;

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

            // FieldId 初期化（次に割り当てる ID）
            if meta_table.get(META_FIELD_ID)?.is_none() {
                meta_table.insert(META_FIELD_ID, 0)?;
            }
        }

        //トランザクションを反映
        write_tx.commit()?;

        Ok(Self { db })
    }

    fn write_begin(&mut self) -> Result<Self::WriteTx, Error> {
        let tx = self.db.begin_write()?;
        Ok(OnDiskWriteTx { inner: tx })
    }

    fn read_begin(&self) -> Result<Self::ReadTx, Error> {
        let tx = self.db.begin_read()?;
        Ok(OnDiskReadTx { inner: tx })
    }
}
