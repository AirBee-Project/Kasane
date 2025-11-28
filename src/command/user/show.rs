#[cfg(feature = "file")]
use std::sync::Arc;

use crate::{interface::output::Output, user_error::UserError};
#[cfg(feature = "file")]
use crate::io::full::Storage;

#[cfg(feature = "file")]
pub fn show_users(s: Arc<&Storage>) -> Result<Output, UserError> {
    s.show_users()
}
