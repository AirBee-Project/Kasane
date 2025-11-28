use std::sync::Arc;

use crate::{
    io::full::Storage,
    interface::{input::PatchValue, output::Output},
    user_error::UserError,
};

pub fn patch_value(v: PatchValue, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
