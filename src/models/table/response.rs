use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::TableDataType;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TableInfoResponse {
    pub name: String,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TableListResponse(pub Vec<TableInfoResponse>);
