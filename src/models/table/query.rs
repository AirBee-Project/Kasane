use std::fmt::Debug;

use kasane_logic::{Coordinate, FlexId, RangeId, SingleId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "data")]
#[allow(dead_code)]
pub enum SpatialId {
    Single(SingleId),
    Flex(FlexId),
    Range(RangeId),
}

#[derive(Debug, Deserialize, ToSchema)]
/// 時空間IDを指定するクエリ
#[allow(dead_code)]
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
    TableFilter(TableFilter),
    SpatialIds(Vec<SpatialId>),
}

#[derive(Debug, Deserialize, ToSchema)]
///Tableを参照して条件を指定する場合
#[allow(dead_code)]
pub struct TableFilter {
    pub name: String,
    pub query: TableFilterType,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "type", content = "condition")]
#[allow(dead_code)]
pub enum TableFilterType {
    Text(TableFilterText),
    Int(TableFilterInt),
    Float(TableFilterFloat),
    Boolean(TableFilterBoolean),
}

#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
pub enum TableFilterBoolean {
    Equal(bool),
}

#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
pub enum Geometry {
    Point {
        zoomlevel: u8,
        coordinate: Coordinate,
    },
    Line {
        zoomlevel: u8,
        points: [Coordinate; 2],
    },
    Triangle {
        zoomlevel: u8,
        points: [Coordinate; 3],
    },
    Sphere {
        zoomlevel: u8,
        radius: f64,
        center: Coordinate,
    },
}
