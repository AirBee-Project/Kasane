use super::TableDataType;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TableInfoResponse {
    #[schema(example = "example_table")]
    pub name: String,
    #[schema(example = TableDataType::Int)]
    pub data_type: TableDataType,
    #[schema(example = 25)]
    pub max_zoom_level: u8,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TableListResponse(pub Vec<TableInfoResponse>);

impl From<crate::models::database::table::Table> for TableInfoResponse {
    fn from(table: crate::models::database::table::Table) -> Self {
        Self {
            name: table.name,
            data_type: table.data_type,
            max_zoom_level: table.max_zoom_level,
        }
    }
}
