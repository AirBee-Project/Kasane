use std::sync::Arc;

use crate::{
    error::Error,
    io::{StorageTrait, full::Storage, tools::range::range},
    json::{input::PatchValue, output::Output},
};

pub fn patch_value(v: PatchValue, s: Arc<Storage>) -> Result<Output, Error> {
    todo!()
}
