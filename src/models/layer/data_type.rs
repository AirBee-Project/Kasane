use redb::{TypeName, Value};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[repr(u8)]
#[derive(Debug, Deserialize, Serialize, ToSchema, Clone, PartialEq, Eq, Hash, Copy)]
/// Layer内の時空間IDに付与する値の型。
pub enum LayerDataType {
    Text = 0,
    Int = 1,
    Float = 2,
    Boolean = 3,
}

impl Value for LayerDataType {
    type SelfType<'a> = LayerDataType;
    type AsBytes<'a> = [u8; 1];

    fn fixed_width() -> Option<usize> {
        Some(1)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        match data[0] {
            0 => LayerDataType::Text,
            1 => LayerDataType::Int,
            2 => LayerDataType::Float,
            3 => LayerDataType::Boolean,
            _ => panic!("invalid LayerDataType"),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        [match value {
            LayerDataType::Text => 0,
            LayerDataType::Int => 1,
            LayerDataType::Float => 2,
            LayerDataType::Boolean => 3,
        }]
    }

    fn type_name() -> redb::TypeName {
        TypeName::new("my_crate::LayerDataType")
    }
}

impl From<LayerDataType> for JsonValueType {
    fn from(value: LayerDataType) -> Self {
        match value {
            LayerDataType::Text => JsonValueType::String,
            LayerDataType::Int | LayerDataType::Float => JsonValueType::Number,
            LayerDataType::Boolean => JsonValueType::Bool,
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
