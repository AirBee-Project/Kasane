use std::sync::Arc;

use crate::{
    error::Error,
    io::{StorageTrait, full::Storage},
    json::{
        input::ShowValues,
        output::{Output, Value},
    },
    r#type::spacetimeid::{DimensionRange, SpaceTimeId},
};

pub fn show_values(v: ShowValues, s: Arc<Storage>) -> Result<Output, Error> {
    todo!()
}
