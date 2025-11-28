use crate::{
    interface::{
        input::{Calculation, Function, Range},
        output::Output,
    },
    io::full::{table_types::value_entry::ValueEntry, Storage},
    user_error::UserError,
};
use kasane_logic::{
    encode_id_set::EncodeIDSet,
    function,
    point::{Coordinate, ECEF},
    space_time_id::SpaceTimeID,
};

impl Storage {
    pub fn insert_value(
        &self,
        space_name: &str,
        key_name: &str,
        range: Range,
        value: ValueEntry,
    ) -> Result<Output, UserError> {
        // Range を処理して EncodeIDSet を取得
        let encode_range = Self::process_range(range)?;
        // value の挿入処理は別途実装

        todo!()
    }

    fn process_range(range: Range) -> Result<EncodeIDSet, UserError> {
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
            } // Range::FilterValue(_) => todo!(),
        }
    }

    /// Function の処理を短縮化
    fn process_function(function: Function) -> Result<EncodeIDSet, UserError> {
        match function {
            Function::Point(point) => {
                let mut set = EncodeIDSet::new();
                set.insert(function::point::point(
                    point.z,
                    Self::check_coordinate(point.point1)?,
                ));
                Ok(set)
            }
            Function::Line(line) => Ok(kasane_logic::function::line::line(
                line.z,
                Self::check_coordinate(line.point1)?,
                Self::check_coordinate(line.point2)?,
            )),
            Function::Triangle(triangle) => Ok(kasane_logic::function::triangle::triangle(
                triangle.z,
                Self::check_coordinate(triangle.point1)?,
                Self::check_coordinate(triangle.point2)?,
                Self::check_coordinate(triangle.point3)?,
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
                    Ok(acc.intersection(&Storage::process_range(r)?))
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

    fn check_coordinate(point: Coordinate) -> Result<Coordinate, UserError> {
        Ok(Coordinate::new(
            point.latitude,
            point.longitude,
            point.altitude,
        )?)
    }
}
