#[cfg(feature = "file")]
use std::sync::Arc;

use crate::{
    command::tools::valid_name::valid_name,
    interface::{input::DropSpace, output::Output},
    user_error::UserError,
};
#[cfg(feature = "file")]
use crate::io::full::Storage;

#[cfg(feature = "file")]
#[allow(unused_variables)]
pub fn drop_space(v: DropSpace, s: Arc<&Storage>) -> Result<Output, UserError> {
    todo!()
}
