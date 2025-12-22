use crate::{
    error::Error,
    io::Kasane,
    transaction::{models::KeyType, read::ReadTxTrait, write::WriteTxTrait},
};
use redb::{ReadableDatabase, TableDefinition, Value};
use std::path::Path;

pub struct OnDisk {
    pub(crate) db: redb::Database,
}

pub struct OnDiskWriteTx {
    pub(crate) inner: redb::WriteTransaction,
}

pub struct OnDiskReadTx {
    pub(crate) inner: redb::ReadTransaction,
}

static KEY_TABLE: TableDefinition<String, KeyInfo> = TableDefinition::new("key");

impl Kasane for OnDisk {
    fn new(path: &Path) -> Result<OnDisk, Error> {
        use redb::Database;
        let db = Database::create(path)?;
        {
            let write_txn = db.begin_write()?;
            write_txn.open_table(KEY_TABLE)?;
            write_txn.commit()?;
        }

        Ok(OnDisk { db })
    }

    fn write_begin(&'_ mut self) -> Result<impl WriteTxTrait, Error> {
        let tx = self.db.begin_write()?;
        Ok(OnDiskWriteTx { inner: tx })
    }

    fn read_begin(&'_ self) -> Result<impl ReadTxTrait, Error> {
        let tx = self.db.begin_read()?;
        Ok(OnDiskReadTx { inner: tx })
    }
}

#[derive(Debug)]
pub struct KeyInfo {
    r#type: KeyType,
    id: u64,
}
impl Value for KeyInfo {
    type SelfType<'a>
        = KeyInfo
    where
        Self: 'a;
    type AsBytes<'a>
        = [u8; 9]
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        Some(9)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        assert_eq!(data.len(), 9);

        let r#type = match data[0] {
            0 => KeyType::Text,
            1 => KeyType::Float,
            2 => KeyType::Int,
            3 => KeyType::Boolean,
            _ => panic!("invalid KeyType"),
        };

        let mut id_bytes = [0u8; 8];
        id_bytes.copy_from_slice(&data[1..9]);
        let id = u64::from_le_bytes(id_bytes);

        KeyInfo { r#type, id }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = [0u8; 9];
        bytes[0] = value.r#type as u8;
        bytes[1..9].copy_from_slice(&value.id.to_le_bytes());
        bytes
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("KeyInfo")
    }
}
