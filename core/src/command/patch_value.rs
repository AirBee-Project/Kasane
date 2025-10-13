use std::sync::Arc;

use crate::{
    user_error::UserError,
    io::{StorageTrait, full::Storage, tools::range::range},
    json::{input::PatchValue, output::Output},
};

pub fn patch_value(v: PatchValue, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
