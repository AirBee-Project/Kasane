use super::TableDataType;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, Deserialize, Copy)]
pub struct TableMetadata {
    pub id: Uuid,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
}
