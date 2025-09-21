use std::sync::Arc;

use kasane_logic::id::{DimensionRange, SpaceTimeId};

use crate::{
    error::Error,
    io::{StorageTrait, full::Storage, tools::range::bitmask_to_id},
    json::{
        input::ShowValues,
        output::{Output, Value},
    },
};

pub fn show_values(v: ShowValues, s: Arc<Storage>) -> Result<Output, Error> {
    let a = s.show_values(&v.space_name, &v.key_name)?;

    let mut result = vec![];

    for ele in a {
        let id = bitmask_to_id(&ele.0);

        let stid = SpaceTimeId::new(
            id.z,
            DimensionRange::Single(id.f),
            DimensionRange::Single(id.x),
            DimensionRange::Single(id.y),
            id.i,
            DimensionRange::Any,
        )
        .unwrap();

        result.push(Value {
            id: stid,
            center: stid.center(),
            vertex: stid.vertex(),
            id_string: stid.to_string(),
            value: ele.1,
        });
    }

    return Ok(Output::ShowValues(result));
}
