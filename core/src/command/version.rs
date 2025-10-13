use crate::{user_error::UserError, json::output::Output};

pub fn version() -> Result<Output, UserError> {
    return Ok(Output::Version(crate::json::output::Version {
        version: env!("CARGO_PKG_VERSION").to_string(),
    }));
}
