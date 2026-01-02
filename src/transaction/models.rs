use std::iter;

use kasane_logic::{
    geometry::{coordinate::Coordinate, shapes::line::line},
    id::space_id::{range::RangeID, single::SingleID},
};

use crate::error::Error;

pub struct Point {
    pub z: u8,
    pub point1: Coordinate,
}

pub struct Line {
    pub z: u8,
    pub point1: Coordinate,
    pub point2: Coordinate,
}

pub enum Range {
    //Logicに投げるべき案件
    Function(Function),

    //Logicに投げる
    Calculation(Calculation),

    //これもLogic
    Ids(Vec<RangeID>),

    //値の有無とVec<u8>のRangeだけは追加
    HasValue {
        field_name: String,
    },

    //Vec<u8>の範囲でFilterできるようにする
    ValueFilter {
        field_name: String,
        filter: std::ops::Range<Vec<u8>>,
    },
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

impl Range {
    pub fn process(self) -> Result<Vec<RangeID>, Error> {
        let ids = match self {
            Range::Function(function) => match function {
                Function::Point(point) => {
                    vec![RangeID::from(point.point1.to_id(point.z))]
                }

                Function::Line(line) => {
                    kasane_logic::geometry::shapes::line::line(line.z, line.point1, line.point2)?
                        .map(RangeID::from)
                        .collect()
                }

                Function::Triangle(triangle) => kasane_logic::geometry::shapes::triangle::triangle(
                    triangle.z,
                    triangle.point1,
                    triangle.point2,
                    triangle.point3,
                )?
                .map(RangeID::from)
                .collect(),
            },
            Range::Calculation(calc) => match calc {
                Calculation::And(ranges) => {
                    todo!()
                }
                Calculation::Or(ranges) => todo!(),
                Calculation::Diff { base, remove } => todo!(),
            },

            //Setを作成する必要がある
            Range::Ids(range_ids) => range_ids,

            Range::HasValue { field_name } => todo!(),
            Range::ValueFilter { field_name, filter } => todo!(),
        };

        Ok(ids)
    }
}
