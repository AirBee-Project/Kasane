use std::collections::HashMap;

use crate::{
    json::input::Range,
    r#type::{space_time_id::SpaceTimeId, space_time_id_set::SpaceTimeIdSet},
    user_error::UserError,
};

//ここで値の検証などを行う
fn range(range: Range) -> Result<SpaceTimeIdSet, UserError> {
    match range {
        Range::Function(function) => todo!(),
        Range::Prefix(prefix) => todo!(),
        Range::Ids(ids) => {
            let mut set = SpaceTimeIdSet::new();
            for id in ids {
                set.insert(SpaceTimeId::new(id.z, id.f, id.x, id.y, id.i, id.t)?);
            }
            return Ok(set);
        }
    }
}
