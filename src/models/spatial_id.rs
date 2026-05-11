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
}

#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SpatialId {
    #[serde(rename = "singleId")]
    SingleId(RawSingleId),
    #[serde(rename = "rangeId")]
    RangeId(RawRangeId),
}
