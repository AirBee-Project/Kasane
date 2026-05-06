use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "data")]
pub enum SpatialId {
    SingleId {
        z: u8,
        f: i32,
        x: u32,
        y: u32,
    },
    RangeId {
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
    // TableFilter(TableFilter),
    SpatialIds(Vec<SpatialId>),
}

#[derive(Debug, Deserialize, ToSchema)]
///Tableを参照して条件を指定する場合
pub struct TableFilter {
    pub name: String,
    pub query: TableFilterType,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "type", content = "condition")]
pub enum TableFilterType {
    Text(TableFilterText),
    Int(TableFilterInt),
    Float(TableFilterFloat),
    Boolean(TableFilterBoolean),
}

#[derive(Debug, Deserialize, ToSchema)]
pub enum TableFilterText {
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
pub enum TableFilterInt {
    Equal(i32),
    NotEqual(i32),
    // GreaterThan(i32),      // >
    // GreaterThanEqual(i32), // >=
    // LessThan(i32),         // <
    // LessThanEqual(i32),    // <=
    // /// 範囲指定 [min, max]
    // Between(i32, i32),
    // /// 指定したいずれかの値に含まれる
    // In(Vec<i32>),
}

#[derive(Debug, Deserialize, ToSchema)]
pub enum TableFilterFloat {
    Equal(f32),
    NotEqual(f32),
    // GreaterThan(f32),
    // GreaterThanEqual(f32),
    // LessThan(f32),
    // LessThanEqual(f32),
    // Between(f32, f32),
}

#[derive(Debug, Deserialize, ToSchema)]
pub enum TableFilterBoolean {
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
