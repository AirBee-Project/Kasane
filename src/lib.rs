#[cfg(feature = "file")]
pub mod command;
pub mod configuration;
pub mod interface;
pub mod io;
pub mod macros;
pub mod user_error;

#[cfg(feature = "file")]
use once_cell::sync::OnceCell;

#[cfg(any(feature = "wasm", feature = "file"))]
use crate::configuration::Configuration;

#[cfg(feature = "file")]
use crate::{
    command::process, interface::input::Command, interface::output::Output, io::full::Storage,
    user_error::UserError,
};

// ---- グローバルインスタンス ----
#[cfg(feature = "file")]
static STORAGE: OnceCell<Box<Storage>> = OnceCell::new();

#[cfg(feature = "wasm")]
pub fn init(_conf: Configuration) {
    // WASM initialization
}

#[cfg(feature = "file")]
pub fn init(_conf: Configuration) {
    // File-based storage initialization would go here
}

#[cfg(feature = "file")]
pub async fn kasane(command: Command) -> Result<Output, UserError> {
    if let Some(storage) = STORAGE.get() {
        process(command, std::sync::Arc::new(storage.as_ref())).await
    } else {
        Err(UserError::DatabaseError {
            message: "Storage not initialized".to_string(),
            location: format!("{}:{}", file!(), line!()),
        })
    }
}
