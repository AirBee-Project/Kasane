#![cfg(all(not(target_arch = "wasm32"), feature = "redb"))]
use super::Backend;
use crate::{
    backend::{redb::roaring_treemap::RedbRoaringTreemap, FieldId},
    Error,
};
use kasane_logic::{FlexIdRank, Segment};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

///Read Transactionの機能を提供する
mod read;
///Redb向けのRoaringTreeMapを提供する
mod roaring_treemap;
///1フィールドをTableLogicに当てはめた挙動を提供する
mod single_field;
///Write Transactionの機能を提供する
mod write;

const FIELD: TableDefinition<String, FieldId> = TableDefinition::new("field");
const META: TableDefinition<&str, u64> = TableDefinition::new("meta");

const MAIN: TableDefinition<
    (FieldId, FlexIdRank),
    (
        [u8; Segment::ARRAY_LENGTH],
        [u8; Segment::ARRAY_LENGTH],
        [u8; Segment::ARRAY_LENGTH],
    ),
> = TableDefinition::new("main");
const F: TableDefinition<(FieldId, [u8; Segment::ARRAY_LENGTH]), RedbRoaringTreemap> =
    TableDefinition::new("f");
const X: TableDefinition<(FieldId, [u8; Segment::ARRAY_LENGTH]), RedbRoaringTreemap> =
    TableDefinition::new("x");
const Y: TableDefinition<(FieldId, [u8; Segment::ARRAY_LENGTH]), RedbRoaringTreemap> =
    TableDefinition::new("y");

const FORWARD: TableDefinition<(FieldId, FlexIdRank), u64> = TableDefinition::new("forward");

// Dictionary: (FieldId, ValueRank) -> Value (bytes)
// ValueRankに対応する実際のデータを記録
const DICTIONARY: TableDefinition<(FieldId, u64), &[u8]> = TableDefinition::new("dictionary");

// Reverse: (FieldId, Value) -> ValueRank
// データからValueRankを逆引き（重複排除用）
const REVERSE: TableDefinition<(FieldId, &[u8]), u64> = TableDefinition::new("reverse");

pub struct RedbBackend {
    db: Database,
}

pub struct RedbReadTx(redb::ReadTransaction);
pub struct RedbWriteTx(redb::WriteTransaction);

impl Backend for RedbBackend {
    type ReadTx<'a> = RedbReadTx;
    type WriteTx<'a> = RedbWriteTx;

    fn new(path: &str) -> Result<Self, Error> {
        let db = Database::create(path)?;
        let txn = db.begin_write()?;
        {
            txn.open_table(FIELD)?;
            txn.open_table(META)?;
            let mut meta = txn.open_table(META)?;
            if meta.get("next_field_id")?.is_none() {
                meta.insert("next_field_id", 1)?;
            }
        }
        txn.commit()?;
        Ok(Self { db })
    }

    fn begin_read(&self) -> Result<Self::ReadTx<'_>, Error> {
        let txn = self.db.begin_read()?;
        Ok(RedbReadTx(txn))
    }

    fn begin_write(&self) -> Result<Self::WriteTx<'_>, Error> {
        let txn = self.db.begin_write()?;
        Ok(RedbWriteTx(txn))
    }
}
