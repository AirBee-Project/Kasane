use serde::Deserialize;
use crate::models::layer::LayerDataType;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct Layer {
    pub id: Uuid,
    pub name: String,
    pub data_type: LayerDataType,
    pub max_zoom_level: u8,
}
