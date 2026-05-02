use redb::{TypeName, Value};

use crate::models::table::TableDataType;

#[derive(Debug, Clone)]
pub struct TableMetadata {
    pub r#type: TableDataType,
}

impl Value for TableMetadata {
    type SelfType<'a> = TableMetadata;

    type AsBytes<'a> = [u8; 1];

    fn fixed_width() -> Option<usize> {
        Some(1)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        TableMetadata {
            r#type: TableDataType::from_bytes(&data[0..1]),
        }
    }
    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        [value.r#type.clone() as u8]
    }

    fn type_name() -> TypeName {
        TypeName::new("my_crate::TableMetadata")
    }
}
