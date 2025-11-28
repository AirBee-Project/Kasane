#[cfg(feature = "file")]
use std::sync::Arc;

use crate::{interface::output::Output, user_error::UserError};
#[cfg(feature = "file")]
use crate::io::full::Storage;

#[cfg(feature = "file")]
pub fn show_spaces(s: Arc<&Storage>) -> Result<Output, UserError> {
    //ストレージに対して操作を実行する

    s.show_spaces()
}
