use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::spatial_id::SpatialId;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetDataResponse {
    pub ids: Vec<SpatialData>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpatialData {
    pub id: SpatialId,
    pub data: serde_json::Value,
}
