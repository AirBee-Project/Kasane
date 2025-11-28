use crate::io::io::Storage;

use crate::{
    interface::{input::PatchValue, output::Output},
    user_error::UserError,
};
use std::sync::Arc;

pub fn patch_value(v: PatchValue, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
