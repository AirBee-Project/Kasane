use crate::{error::Error, json::output::Output};

pub fn version() -> Result<Output, Error> {
    return Ok(Output::Version(crate::json::output::Version {
        version: env!("CARGO_PKG_VERSION").to_string(),
    }));
}
