use kasane_logic::{geometry::coordinate::Coordinate, id::space_id::range::RangeID};

use crate::{error::Error, location};
pub enum Range {
    Function(Function),
    Calculation(Calculation),
    Ids(Vec<RangeID>),
    FilterValue(FilterValue),
}

pub struct Point {
    pub z: u8,
    pub point1: Coordinate,
}

pub struct Line {
    pub z: u8,
    pub point1: Coordinate,
    pub point2: Coordinate,
}

pub struct Triangle {
    pub z: u8,
    pub point1: Coordinate,
    pub point2: Coordinate,
    pub point3: Coordinate,
}

pub struct FilterValue {
    pub key_name: String,
    pub filter: Filter,
}

pub enum Filter {
    FilterBoolean(FilterBoolean),
    FilterInt(FilterInt),
    FilterFloat(FilterFloat),
    FilterText(FilterText),
}

pub enum FilterBoolean {
    HasValue,
    IsTrue,
    IsFalse,
    Equals(bool),
    NotEquals(bool),
}

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

pub enum Function {
    Point(Point),
    Line(Line),
    Triangle(Triangle),
}

pub enum Calculation {
    And(Vec<Range>),
    Or(Vec<Range>),
    Diff {
        base: Box<Range>,   // 引かれる集合（元集合）
        remove: Box<Range>, // 引く集合
    },
}

///そのフィールドの型を表現する
/// 番号は上から予約していく
#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum FieldType {
    Text = 0,
    Float = 1,
    Int = 2,
    Boolean = 3,
}

impl From<FieldType> for u8 {
    fn from(value: FieldType) -> Self {
        value as u8
    }
}

impl TryFrom<u8> for FieldType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(FieldType::Text),
            1 => Ok(FieldType::Float),
            2 => Ok(FieldType::Int),
            3 => Ok(FieldType::Boolean),
            _ => Err(Error::DataCorruption {
                location: location!(),
                kind: crate::error::DataCorruptionKind::InvalidFieldType,
            }),
        }
    }
}
