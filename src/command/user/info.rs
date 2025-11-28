#[cfg(feature = "file")]
use std::sync::Arc;

use crate::{
    interface::{input::InfoUser, output::Output},
    user_error::UserError,
};
#[cfg(feature = "file")]
use crate::io::full::Storage;

#[cfg(feature = "file")]
pub fn info_user(v: InfoUser, s: Arc<&Storage>) -> Result<Output, UserError> {
    s.info_user(&v.user_name)
}
