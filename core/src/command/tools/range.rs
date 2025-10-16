use crate::{
    json::input::{Id, Range},
    r#type::spacetimeid::z_range::{F_MAX, F_MIN, XY_MAX},
    user_error::UserError,
};

pub fn range(range: Range) -> Result<Range, UserError> {
    match range {
        Range::Function(function) => match function {
            crate::json::input::Function::Spot(spot) => todo!(),
            crate::json::input::Function::Line(line) => todo!(),
            crate::json::input::Function::Triangle(triangle) => todo!(),
        },
        Range::Prefix(prefix) => match prefix {
            crate::json::input::Prefix::AND(ranges) => todo!(),
            crate::json::input::Prefix::OR(ranges) => todo!(),
            crate::json::input::Prefix::NOT(ranges) => todo!(),
        },
        Range::Ids(ids) => {
            for id in ids {
                //ZoomLeveLは規定値内か？
                if id.z > 60 {
                    return Err(UserError::ZoomLevelOutOfRange {
                        zoom_level: id.z,
                        id: (),
                        location: location!(),
                    });
                }

                //各次元について検証

                //
            }
        }
    }
    todo!()
}
