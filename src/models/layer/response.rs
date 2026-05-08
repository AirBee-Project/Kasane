use super::LayerDataType;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct LayerInfoResponse {
    #[schema(example = "example_layer")]
    pub name: String,
    #[schema(example = LayerDataType::Int)]
    pub data_type: LayerDataType,
    #[schema(example = 25)]
    pub max_zoom_level: u8,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct LayerListResponse(pub Vec<LayerInfoResponse>);
