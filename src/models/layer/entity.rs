use super::LayerDataType;
use redb::Value;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Copy)]
pub struct LayerMetadata {
    pub id: Uuid,
    pub data_type: LayerDataType,
    pub max_zoom_level: u8,
}

impl Value for LayerMetadata {
    type SelfType<'a> = LayerMetadata;
    type AsBytes<'a> = [u8; 18];

    fn fixed_width() -> Option<usize> {
        Some(18)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let id = Uuid::from_slice(&data[0..16]).expect("invalid uuid bytes");
        let data_type = LayerDataType::from_bytes(&data[16..17]);
        let max_zoom_level = *data[17..18].first().unwrap();
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
        let mut bytes = [0u8; 18];
        bytes[0..16].copy_from_slice(value.id.as_bytes());
        bytes[16] = value.data_type as u8;
        bytes[17] = value.max_zoom_level;
        bytes
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("my_crate::LayerMetadata")
    }
}
