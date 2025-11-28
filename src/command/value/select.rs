#[cfg(feature = "file")]
use std::sync::Arc;

use crate::{
    user_error::UserError,
    interface::{input::SelectValue, output::Output},
};
#[cfg(feature = "file")]
use crate::io::full::Storage;

#[cfg(feature = "file")]
#[allow(unused_variables)]
pub fn select_value(v: SelectValue, s: Arc<&Storage>) -> Result<Output, UserError> {
    todo!()
}
