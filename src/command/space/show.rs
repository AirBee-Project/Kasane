use crate::io::io::Storage;

use crate::{interface::output::Output, user_error::UserError};
use std::sync::Arc;

pub fn show_spaces(s: Arc<Storage>) -> Result<Output, UserError> {
    s.show_spaces()
}
