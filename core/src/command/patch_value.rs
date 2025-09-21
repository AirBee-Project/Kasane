use std::sync::Arc;

use crate::{
    error::Error,
    io::{StorageTrait, full::Storage, tools::range::range},
    json::{input::PatchValue, output::Output},
};

pub fn patch_value(v: PatchValue, s: Arc<Storage>) -> Result<Output, Error> {
    let range = match range(v.range) {
        Ok(v) => v,
        Err(e) => {
            return Err(Error::RangeError { message: e });
        }
    };
    s.patch_value(&v.space_name, &v.key_name, range, v.value)
}
