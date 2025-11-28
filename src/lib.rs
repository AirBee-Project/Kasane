pub mod command;
pub mod configuration;
pub mod interface;
pub mod io;
pub mod macros;
pub mod user_error;

use std::process::Command;

use once_cell::sync::OnceCell;

use crate::{
    command::process, configuration::Configuration, interface::output::Output, io::Storage,
    user_error::UserError,
};

// ---- グローバルインスタンス ----
static STORAGE: OnceCell<Box<dyn Storage + Send + Sync>> = OnceCell::new();

pub fn init(conf: Configuration) {
    #[cfg(feature = "wasm")]
    STORAGE.set(Box::new(Storage::new())).unwrap();
}

pub fn kasane(command: Command) -> Result<Output, UserError> {
    process(command, STORAGE)
}
