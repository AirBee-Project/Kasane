pub mod command;
pub mod configuration;
pub mod interface;
pub mod io;
pub mod macros;
pub mod user_error;

#[cfg(feature = "wasm")]
use once_cell::sync::OnceCell;
use std::sync::Arc;

#[cfg(feature = "wasm")]
use crate::{
    configuration::Configuration,
    interface::{input::Command, output::Output},
    io::wasm::Storage,
    user_error::UserError,
};

#[cfg(feature = "wasm")]
static STORAGE: OnceCell<Arc<Storage>> = OnceCell::new();

#[cfg(feature = "wasm")]
pub fn init(conf: Configuration) {
    let storage = Arc::new(Storage::new(conf));
    STORAGE.set(storage).expect("Storage already initialized");
}

#[cfg(feature = "wasm")]
pub async fn kasane(command: Command) -> Result<Output, UserError> {
    let s = STORAGE.get().expect("storage not initialized").clone();
    command::process(command, s)
}
