use crate::models::database::table::TableDataType;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct Table {
    pub id: Uuid,
    pub name: String,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
}
