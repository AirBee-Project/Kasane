use redb::{TypeName, Value};

use crate::models::table::TableDataType;

#[derive(Debug, Clone)]
pub struct TableMetadata {
    pub rank: u64,
    pub r#type: TableDataType,
    pub max_zoom_level: u8,
}

impl Value for TableMetadata {
    type SelfType<'a> = TableMetadata;

    type AsBytes<'a> = [u8; 10];

    fn fixed_width() -> Option<usize> {
        Some(10)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let rank = u64::from_le_bytes(data[0..8].try_into().expect("invalid u64 bytes"));
        let r#type = TableDataType::from_bytes(&data[8..9]);
        let max_zoom_level = data[9..10].first().unwrap().clone();

        TableMetadata {
            rank,
            r#type,
            max_zoom_level,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = [0u8; 10];

        bytes[0..8].copy_from_slice(&value.rank.to_le_bytes());
        bytes[8] = value.r#type.clone() as u8;
        bytes[9] = value.max_zoom_level;

        bytes
    }

    fn type_name() -> TypeName {
        TypeName::new("my_crate::TableMetadata")
    }
}
