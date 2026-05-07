use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "data")]
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

#[derive(Debug, Deserialize, ToSchema)]
/// 時空間IDを指定するクエリ
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
    GeometryQuery(Geometry),
    // LayerFilter(LayerFilter),
    SpatialIds(Vec<SpatialId>),
}

#[derive(Debug, Deserialize, ToSchema)]
///Layerを参照して条件を指定する場合
pub struct LayerFilter {
    pub name: String,
    pub query: LayerFilterType,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "type", content = "condition")]
pub enum LayerFilterType {
    Text(LayerFilterText),
    Int(LayerFilterInt),
    Float(LayerFilterFloat),
    Boolean(LayerFilterBoolean),
}

#[derive(Debug, Deserialize, ToSchema)]
pub enum LayerFilterText {
    /// 等しい
    Equal(String),
    /// 等しくない
    NotEqual(String),
    /// 前方一致 (B-Tree の range(prefix..) で高速に処理可能)
    StartsWith(String),
    /// 指定したいずれかの文字列に含まれる
    In(Vec<String>),
}

#[derive(Debug, Deserialize, ToSchema)]
pub enum LayerFilterInt {
    Equal(i32),
    NotEqual(i32),
}

#[derive(Debug, Deserialize, ToSchema)]
pub enum LayerFilterFloat {
    Equal(f32),
    NotEqual(f32),
}

#[derive(Debug, Deserialize, ToSchema)]
pub enum LayerFilterBoolean {
    Equal(bool),
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PointCoordinate {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
}

#[derive(Debug, Deserialize, ToSchema)]
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
