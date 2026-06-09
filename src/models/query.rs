use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use utoipa::ToSchema;

use crate::models::spatial_id::SpatialId;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Query {
    SpatialIds {
        ids: Vec<SpatialId>,
    },
    Geometry {
        geometry: Geometry,
    },
    // TableFilter {
    //     filter: TableFilter,
    // },
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
pub struct TableFilter {
    pub name: String,
    pub query: TableFilterType,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum TableFilterType {
    Text(TableFilterText),
    Int(TableFilterInt),
    Float(TableFilterFloat),
    Boolean(TableFilterBoolean),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", content = "value", rename_all = "camelCase")]
pub enum TableFilterText {
    Equal(String),
    NotEqual(String),
    StartsWith(String),
    In(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", content = "value", rename_all = "camelCase")]
pub enum TableFilterInt {
    Equal(i32),
    NotEqual(i32),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", content = "value", rename_all = "camelCase")]
pub enum TableFilterFloat {
    Equal(f32),
    NotEqual(f32),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", content = "value", rename_all = "camelCase")]
pub enum TableFilterBoolean {
    Equal(bool),
}
