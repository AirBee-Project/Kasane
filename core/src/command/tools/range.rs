use crate::{json::input::Range, r#type::space_time_id::SpaceTimeId, user_error::UserError};

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
                //時空間IDをエンコード
                let encode_id = SpaceTimeId::new(id.z, id.f, id.x, id.y, id.i, id.t)?;

                //重複している範囲をmerge？

                //
            }
        }
    }
    todo!()
}
