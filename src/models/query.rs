use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SpatialId {
    SingleId {
        #[schema(example = 25, maximum = 30)]
        z: u8,
        f: i32,
        x: u32,
        y: u32,
    },
    RangeId {
        #[schema(example = 25, maximum = 30)]
        z: u8,
        f: [i32; 2],
        x: [u32; 2],
        y: [u32; 2],
    },
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Query {
    Union {
        #[schema(no_recursion)]
        left: Box<Self>,
        #[schema(no_recursion)]
        right: Box<Self>,
    },
    Intersection {
        #[schema(no_recursion)]
        left: Box<Self>,
        #[schema(no_recursion)]
        right: Box<Self>,
    },
    Difference {
        #[schema(no_recursion)]
        base: Box<Self>,
        #[schema(no_recursion)]
        subtract: Box<Self>,
    },
    Geometry {
        geometry: Geometry,
    },
    // LayerFilter {
    //     filter: LayerFilter,
    // },
    SpatialIds {
        ids: Vec<SpatialId>,
    },
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Geometry {
    Coordinate {
        zoomlevel: u8,
        coordinate: PointCoordinate,
    },
    Line {
        zoomlevel: u8,
        points: [PointCoordinate; 2],
    },
    Triangle {
        zoomlevel: u8,
        points: [PointCoordinate; 3],
    },
    Sphere {
        zoomlevel: u8,
        radius_m: f64,
        center: PointCoordinate,
    },
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PointCoordinate {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LayerFilter {
    pub name: String,
    pub query: LayerFilterType,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum LayerFilterType {
    Text(LayerFilterText),
    Int(LayerFilterInt),
    Float(LayerFilterFloat),
    Boolean(LayerFilterBoolean),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", content = "value", rename_all = "camelCase")]
pub enum LayerFilterText {
    Equal(String),
    NotEqual(String),
    StartsWith(String),
    In(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", content = "value", rename_all = "camelCase")]
pub enum LayerFilterInt {
    Equal(i32),
    NotEqual(i32),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", content = "value", rename_all = "camelCase")]
pub enum LayerFilterFloat {
    Equal(f32),
    NotEqual(f32),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", content = "value", rename_all = "camelCase")]
pub enum LayerFilterBoolean {
    Equal(bool),
}
