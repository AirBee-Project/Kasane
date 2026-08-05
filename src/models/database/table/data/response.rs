use crate::models::spatial_id::{RawFlexId, RawRangeId, RawSingleId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum GetDataResponse {
    Single(GetDataResponseSingle),
    Range(GetDataResponseRange),
    Flex(GetDataResponseFlex),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetDataResponseSingle {
    pub dictionary: Vec<serde_json::Value>,
    pub data: Vec<DataGroup<RawSingleId>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetDataResponseRange {
    pub dictionary: Vec<serde_json::Value>,
    pub data: Vec<DataGroup<RawRangeId>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetDataResponseFlex {
    pub dictionary: Vec<serde_json::Value>,
    pub data: Vec<DataGroup<RawFlexId>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DataGroup<T> {
    pub value_ref: usize,
    pub spatial_ids: Vec<T>,
}
