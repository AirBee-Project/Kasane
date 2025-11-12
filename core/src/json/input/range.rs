use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ---------------------- Range & Function ----------------------

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub enum Range {
    Function(Function),
    Prefix(Prefix),
    Ids(Vec<SpaceTimeIdInput>),
    //FilterValue(FilterValue),
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpaceTimeIdInput {
    pub z: u8,
    pub f: (Option<i64>, Option<i64>),
    pub x: (Option<u64>, Option<u64>),
    pub y: (Option<u64>, Option<u64>),
    pub i: u32,
    pub t: (Option<u32>, Option<u32>),
}

// #[derive(Debug, Serialize, Deserialize, Clone)]
// #[serde(rename_all = "camelCase")]
// pub struct Spot {
//     pub point1: Point,
//     pub zoom: u8,
// }

// #[derive(Debug, Serialize, Deserialize, Clone)]
// #[serde(rename_all = "camelCase")]
// pub struct Line {
//     pub point1: Point,
//     pub point2: Point,
//     pub zoom: u8,
// }

// #[derive(Debug, Serialize, Deserialize, Clone)]
// #[serde(rename_all = "camelCase")]
// pub struct Triangle {
//     pub point1: Point,
//     pub point2: Point,
//     pub point3: Point,
//     pub zoom: u8,
// }

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct FilterValue {
    pub space_name: String,
    pub key_name: String,
    pub filter: Filter,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub enum Filter {
    FilterBoolean(FilterBoolean),
    FilterInt(FilterInt),
    FilterFloat(FilterFloat),
    FilterText(FilterText),
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub enum FilterBoolean {
    HasValue,
    IsTrue,
    IsFalse,
    Equals(bool),
    NotEquals(bool),
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub enum FilterFloat {
    HasValue,
    Equal(f32),
    NotEqual(f32),
    GreaterThan(f32),
    GreaterEqual(f32),
    LessThan(f32),
    LessEqual(f32),
    Between(f32, f32),
    In(Vec<f32>),
    NotIn(Vec<f32>),
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub enum FilterInt {
    HasValue,
    Equal(i32),
    NotEqual(i32),
    GreaterThan(i32),
    GreaterEqual(i32),
    LessThan(i32),
    LessEqual(i32),
    Between(i32, i32),
    In(Vec<i32>),
    NotIn(Vec<i32>),
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub enum FilterText {
    HasValue,
    Equal(String),
    NotEqual(String),
    Contains(String),
    NotContains(String),
    StartsWith(String),
    EndsWith(String),
    CaseInsensitiveEqual(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub enum Function {
    // Spot(Spot),
    // Line(Line),
    // Triangle(Triangle),
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub enum Prefix {
    AND(Vec<Range>),
    OR(Vec<Range>),
    NOT(Box<Range>),
}
