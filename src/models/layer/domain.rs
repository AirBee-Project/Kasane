use serde::Deserialize;
use crate::models::layer::LayerDataType;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub data_type: LayerDataType,
    pub max_zoom_level: u8,
}
