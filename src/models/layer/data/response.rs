use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::spatial_id::SingleId;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetDataResponse {
    pub ids: Vec<SpatialData>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpatialData {
    // 現状ではSingleIdでしか返さない
    pub id: SingleId,
    pub data: serde_json::Value,
}
