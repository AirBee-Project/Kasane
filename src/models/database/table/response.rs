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
    #[schema(example = 100)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TableListResponse(pub Vec<TableInfoResponse>);
