use crate::models::database::table::TableDataType;
use crate::models::database::table::data_type::TableConstraints;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Table {
    pub id: crate::models::id::TableId,
    pub name: String,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
    pub constraints: Option<TableConstraints>,
    pub description: Option<String>,
}
