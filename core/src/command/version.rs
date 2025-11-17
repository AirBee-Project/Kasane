use std::sync::Arc;

use crate::{io::full::Storage, interface::output::Output, user_error::UserError};

pub fn version(s: Arc<Storage>) -> Result<Output, UserError> {
    return Ok(Output::Version(crate::interface::output::Version {
        version: env!("CARGO_PKG_VERSION").to_string(),
    }));
}
