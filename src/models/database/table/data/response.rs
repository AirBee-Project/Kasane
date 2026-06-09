use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::spatial_id::RawSingleId;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetDataResponse {
    pub ids: Vec<SpatialData>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpatialData {
    // 現状ではSingleIdでしか返さない
    // 招待的にはデータ量の圧縮のため、[RangeId]や[FlexId]を導入する
    pub id: RawSingleId,
    pub data: serde_json::Value,
}
