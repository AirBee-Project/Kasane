use super::TableDataType;
use crate::models::database::table::data_type::TableConstraints;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, serde::Serialize, Deserialize)]
pub struct TableMetadata {
    pub id: crate::models::id::TableId,
    pub data_type: TableDataType,
    pub constraints: Option<TableConstraints>,
}
