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
pub struct StreamDictionaryEvent {
    pub value_ref: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamDataEventSingle {
    pub value_ref: String,
    pub spatial_ids: Vec<RawSingleId>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StreamEventSingle {
    Dictionary(StreamDictionaryEvent),
    Data(StreamDataEventSingle),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamDataEventRange {
    pub value_ref: String,
    pub spatial_ids: Vec<RawRangeId>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StreamEventRange {
    Dictionary(StreamDictionaryEvent),
    Data(StreamDataEventRange),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamDataEventFlex {
    pub value_ref: String,
    pub spatial_ids: Vec<RawFlexId>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StreamEventFlex {
    Dictionary(StreamDictionaryEvent),
    Data(StreamDataEventFlex),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DataGroup<T> {
    pub value_ref: usize,
    pub spatial_ids: Vec<T>,
}
