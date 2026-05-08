use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SpatialId {
    SingleId {
        #[schema(example = 20, maximum = 30)]
        z: u8,
        #[schema(example = 0)]
        f: i32,
        #[schema(example = 931386)]
        x: u32,
        #[schema(example = 412905)]
        y: u32,
    },
    RangeId {
        #[schema(example = 20, maximum = 30)]
        z: u8,
        #[schema(example = json!([0,0]))]
        f: [i32; 2],
        #[schema(example = json!([931388,931390]))]
        x: [u32; 2],
        #[schema(example = json!([412900,412907]))]
        y: [u32; 2],
    },
}
