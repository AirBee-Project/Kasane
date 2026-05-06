use kasane_logic::SingleId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::table::entity::TableMetadata;

use super::TableDataType;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct InfoTableResponse {
    pub name: String,
    pub r#type: TableDataType,
    pub max_zoom_level: u8,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GetValueResponse {
    pub ids: Vec<(SingleId, serde_json::Value)>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GetTableListResponse {
    pub tables: Vec<(String, TableMetadata)>,
}
