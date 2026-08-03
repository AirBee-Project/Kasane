use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Clone, Hash, Eq)]
#[schema(as = SingleId)]
pub struct RawSingleId {
    #[schema(example = 20, maximum = 30)]
    pub z: u8,
    #[schema(example = 0)]
    pub f: i32,
    #[schema(example = 931386)]
    pub x: u32,
    #[schema(example = 412905)]
    pub y: u32,
    #[schema(example = 3600, minimum = 1)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i: Option<u64>,
    #[schema(example = 1)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Clone)]
#[schema(as = RangeId)]
pub struct RawRangeId {
    #[schema(example = 20, maximum = 30)]
    pub z: u8,
    #[schema(example = json!([0,0]))]
    pub f: [i32; 2],
    #[schema(example = json!([931388,931390]))]
    pub x: [u32; 2],
    #[schema(example = json!([412900,412907]))]
    pub y: [u32; 2],
    #[schema(example = 3600, minimum = 1)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i: Option<u64>,
    #[schema(example = json!([0, 5]))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<[u64; 2]>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Clone)]
#[schema(as = FlexId)]
#[serde(rename_all = "camelCase")]
pub struct RawFlexId {
    #[schema(example = 20, maximum = 30)]
    pub f_zoomlevel: u8,
    #[schema(example = 0)]
    pub f_index: i32,
    #[schema(example = 20, maximum = 30)]
    pub x_zoomlevel: u8,
    #[schema(example = 931386)]
    pub x_index: u32,
    #[schema(example = 20, maximum = 30)]
    pub y_zoomlevel: u8,
    #[schema(example = 412905)]
    pub y_index: u32,
    #[schema(example = 3600, minimum = 1)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i: Option<u64>,
    #[schema(example = 1)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SpatialId {
    #[serde(rename = "singleId")]
    SingleId(RawSingleId),
    #[serde(rename = "rangeId")]
    RangeId(RawRangeId),
    #[serde(rename = "flexId")]
    FlexId(RawFlexId),
}
