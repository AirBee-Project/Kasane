use serde::Deserialize;

use crate::models::table::TableDataType;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct Table {
    pub id: u64,
    pub name: String,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
}

