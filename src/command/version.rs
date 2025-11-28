#[cfg(feature = "file")]
use std::sync::Arc;

use crate::{interface::output::Output, user_error::UserError};
#[cfg(feature = "file")]
use crate::io::full::Storage;

#[cfg(feature = "file")]
pub fn version(_s: Arc<&Storage>) -> Result<Output, UserError> {
    Ok(Output::Version(crate::interface::output::Version {
        version: env!("CARGO_PKG_VERSION").to_string(),
    }))
}

#[cfg(not(feature = "file"))]
pub fn version() -> Result<Output, UserError> {
    Ok(Output::Version(crate::interface::output::Version {
        version: env!("CARGO_PKG_VERSION").to_string(),
    }))
}
