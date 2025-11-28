#[cfg(feature = "file")]
use std::sync::Arc;

use crate::{
    interface::{input::DropUser, output::Output},
    user_error::UserError,
};
#[cfg(feature = "file")]
use crate::io::full::Storage;

#[cfg(feature = "file")]
pub fn drop_user(v: DropUser, s: Arc<&Storage>) -> Result<Output, UserError> {
    s.drop_user(&v.user_name)
}
