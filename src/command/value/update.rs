#[cfg(feature = "file")]
use std::sync::Arc;

use crate::{
    interface::{
        input::UpdateValue,
        output::Output,
    },
    user_error::UserError,
};
#[cfg(feature = "file")]
use crate::io::full::Storage;

#[cfg(feature = "file")]
#[allow(unused_variables)]
pub fn update_value(v: UpdateValue, s: Arc<&Storage>) -> Result<Output, UserError> {
    todo!()
}
