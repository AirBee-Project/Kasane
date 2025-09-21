use std::sync::Arc;

use crate::{
    error::Error,
    io::{StorageTrait, full::Storage, tools::range::range},
    json::{
        input::{InsertValue, UpdateValue},
        output::Output,
    },
};

pub fn update_value(v: UpdateValue, s: Arc<Storage>) -> Result<Output, Error> {
    let range = match range(v.range) {
        Ok(v) => v,
        Err(e) => {
            return Err(Error::RangeError { message: e });
        }
    };
    s.update_value(&v.space_name, &v.key_name, range, v.value)
}
