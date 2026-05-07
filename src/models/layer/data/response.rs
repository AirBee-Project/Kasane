use kasane_logic::{RangeId, SingleId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetDataResponse {
    pub ids: Vec<SpatialData>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpatialData {
    pub id: ResponseSpatialId,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ResponseSpatialId {
    SingleId(SingleId),
    RangeId(RangeId),
}
