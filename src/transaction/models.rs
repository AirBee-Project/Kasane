use kasane_logic::{geometry::coordinate::Coordinate, id::space_id::range::RangeID};

use crate::{error::Error, location};
pub enum Range {
    Function(Function),
    Calculation(Calculation),
    Ids(Vec<RangeID>),
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
