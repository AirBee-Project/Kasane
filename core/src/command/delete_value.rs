use std::sync::Arc;

use crate::{
    error::Error,
    io::{StorageTrait, full::Storage, tools::range::range},
    json::{input::DeleteValue, output::Output},
};

pub fn delete_value(v: DeleteValue, s: Arc<Storage>) -> Result<Output, Error> {
    let range = match range(v.range) {
        Ok(v) => v,
        Err(e) => {
            return Err(Error::RangeError { message: e });
        }
    };
    s.delete_value(&v.space_name, &v.key_name, range)
}
