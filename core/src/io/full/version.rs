use crate::{io::full::Storage, json::output::Output, user_error::UserError};

impl Storage {
    pub fn version(&self) -> Result<Output, UserError> {
        return Ok(Output::Version(crate::json::output::Version {
            version: env!("CARGO_PKG_VERSION").to_string(),
        }));
    }
}
