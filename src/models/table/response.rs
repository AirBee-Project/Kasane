use kasane_logic::SingleId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
