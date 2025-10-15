use std::sync::Arc;

use crate::{io::full::Storage, json::output::Output, user_error::UserError};

pub fn version(s: Arc<Storage>) -> Result<Output, UserError> {
    s.version()
}
