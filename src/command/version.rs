use crate::{interface::output::Output, user_error::UserError};

pub fn version() -> Result<Output, UserError> {
    Ok(Output::Version(crate::interface::output::Version {
        version: env!("CARGO_PKG_VERSION").to_string(),
    }))
}
