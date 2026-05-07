use super::LayerDataType;
use redb::Value;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Copy)]
pub struct LayerMetadata {
    pub id: u64,
    pub data_type: LayerDataType,
    pub max_zoom_level: u8,
}

impl Value for LayerMetadata {
    type SelfType<'a> = LayerMetadata;
    type AsBytes<'a> = [u8; 10];

    fn fixed_width() -> Option<usize> {
        Some(10)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let id = u64::from_le_bytes(data[0..8].try_into().expect("invalid u64 bytes"));
        let data_type = LayerDataType::from_bytes(&data[8..9]);
        let max_zoom_level = data[9..10].first().unwrap().clone();
        LayerMetadata {
            id,
            data_type,
            max_zoom_level,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = [0u8; 10];
        bytes[0..8].copy_from_slice(&value.id.to_le_bytes());
        bytes[8] = value.data_type.clone() as u8;
        bytes[9] = value.max_zoom_level;
        bytes
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("my_crate::LayerMetadata")
    }
}
