use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[repr(u8)]
#[derive(Debug, Deserialize, Serialize, ToSchema, Clone, PartialEq, Eq, Hash, Copy)]
/// Table内の時空間IDに付与する値の型。
pub enum TableDataType {
    Text = 0,
    Int = 1,
    Float = 2,
    Boolean = 3,
}

impl From<TableDataType> for JsonValueType {
    fn from(value: TableDataType) -> Self {
        match value {
            TableDataType::Text => JsonValueType::String,
            TableDataType::Int | TableDataType::Float => JsonValueType::Number,
            TableDataType::Boolean => JsonValueType::Bool,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsonValueType {
    String,
    Number,
    Bool,
    Array,
    Object,
    Null,
}
