use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use super::LayerDataType;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct LayerInfoResponse {
    pub name: String,
    pub data_type: LayerDataType,
    pub max_zoom_level: u8,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct LayerListResponse(pub Vec<LayerInfoResponse>);
