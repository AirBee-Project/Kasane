use crate::models::database::table::TableDataType;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct Table {
    pub id: crate::models::id::TableId,
    pub name: String,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
}
