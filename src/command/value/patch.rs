#[cfg(feature = "file")]
use std::sync::Arc;

use crate::{
    interface::{input::PatchValue, output::Output},
    user_error::UserError,
};
#[cfg(feature = "file")]
use crate::io::full::Storage;

#[cfg(feature = "file")]
#[allow(unused_variables)]
pub fn patch_value(v: PatchValue, s: Arc<&Storage>) -> Result<Output, UserError> {
    todo!()
}
