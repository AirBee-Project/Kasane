use kasane_logic::{
    encode_id_set::EncodeIDSet, function, point::Coordinate, space_time_id::SpaceTimeID,
};

use crate::{
    interface::input::{Calculation, Function, Range},
    io::wasm::Storage,
    user_error::UserError,
};

impl Storage {
    pub fn process_range(range: Range) -> Result<EncodeIDSet, UserError> {
        match range {
            Range::Function(f) => Self::process_function(f),
            Range::Calculation(c) => Self::process_calculation(c),
            Range::Ids(ids) => {
                let mut set = EncodeIDSet::new();
                for input_id in ids {
                    let id = SpaceTimeID::new(input_id.z, input_id.f, input_id.x, input_id.y)?;
                    for encode_id in id.to_encode() {
                        set.insert(encode_id);
                    }
                }
                Ok(set)
            }
            Range::FilterValue(_) => todo!(),
        }
    }

    /// SpatialPoint -> Coordinate 変換
    fn check_coordinate(point: Coordinate) -> Result<Coordinate, UserError> {
        Ok(Coordinate::new(
            point.latitude,
            point.latitude,
            point.altitude,
        )?)
    }

    /// Function の処理を短縮化
    fn process_function(function: Function) -> Result<EncodeIDSet, UserError> {
        match function {
            Function::Point(p) => {
                let mut set = EncodeIDSet::new();
                set.insert(function::point::point(
                    p.z,
                    Self::check_coordinate(p.point1)?,
                ));
                Ok(set)
            }
            Function::Line(l) => Ok(function::line::line(
                l.z,
                Self::check_coordinate(l.point1)?,
                Self::check_coordinate(l.point2)?,
            )),
            Function::Triangle(t) => Ok(function::triangle::triangle(
                t.z,
                Self::check_coordinate(t.point1)?,
                Self::check_coordinate(t.point2)?,
                Self::check_coordinate(t.point3)?,
            )),
        }
    }

    /// Calculation の集合演算を再帰的に処理
    fn process_calculation(calc: Calculation) -> Result<EncodeIDSet, UserError> {
        match calc {
            Calculation::AND(ranges) => {
                let mut iter = ranges.into_iter();
                let first = match iter.next() {
                    Some(r) => Storage::process_range(r)?,
                    None => return Ok(EncodeIDSet::new()),
                };
                iter.try_fold(first, |acc, r| {
                    Ok(acc.intersection(&Self::process_range(r)?))
                })
            }
            Calculation::OR(ranges) => {
                let mut set = EncodeIDSet::new();
                for r in ranges {
                    set = set.union(&Storage::process_range(r)?);
                }
                Ok(set)
            }
            Calculation::DIFF { base, remove } => {
                let base_set = Storage::process_range(*base)?;
                let remove_set = Storage::process_range(*remove)?;
                Ok(base_set.difference(&remove_set))
            }
        }
    }
}
