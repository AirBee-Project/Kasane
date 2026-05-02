use redb::{TypeName, Value};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[repr(u8)]
#[derive(Debug, Deserialize, Serialize, ToSchema, Clone)]
///Table内の時空間IDに付与する値の型を指定する
/// 型の名前はMySQLと同じ命名規則を採用
pub enum TableDataType {
    ///Rustの[String]に対応
    Text = 0,
    ///Rustの[i32]に対応
    Int = 1,
    ///Rustの[f32]に対応
    Float = 2,
    ///Rustの[bool]に対応
    Boolean = 3,
}

impl Value for TableDataType {
    type SelfType<'a> = TableDataType;

    type AsBytes<'a> = [u8; 1];

    fn fixed_width() -> Option<usize> {
        Some(1)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        match data[0] {
            0 => TableDataType::Text,
            1 => TableDataType::Int,
            2 => TableDataType::Float,
            3 => TableDataType::Boolean,
            _ => panic!("invalid TableDataType"),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let byte = match value {
            TableDataType::Text => 0,
            TableDataType::Int => 1,
            TableDataType::Float => 2,
            TableDataType::Boolean => 3,
        };

        [byte]
    }

    fn type_name() -> redb::TypeName {
        TypeName::new("my_crate::TableDataType")
    }
}
