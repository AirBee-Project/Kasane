pub mod command;
pub mod configuration;
pub mod interface;
pub mod io;
pub mod macros;
pub mod user_error;

#[cfg(feature = "wasm")]
use once_cell::sync::OnceCell;
#[cfg(feature = "wasm")]
use std::sync::RwLock;

#[cfg(feature = "wasm")]
use crate::{
    configuration::Configuration,
    interface::{input::Command, output::Output},
    io::wasm::Storage,
    user_error::UserError,
};

#[cfg(feature = "wasm")]
static STORAGE: OnceCell<RwLock<Storage>> = OnceCell::new();

#[cfg(feature = "wasm")]
pub fn init(conf: Configuration, import: Option<Vec<Storage>>) {
    let storage = RwLock::new(Storage::new(conf, import));
    STORAGE.set(storage).expect("Storage already initialized");
}

#[cfg(feature = "wasm")]
pub fn kasane(command: Command) -> Result<Output, UserError> {
    let mut s = STORAGE
        .get()
        .expect("storage not initialized")
        .write()
        .expect("storage lock poisoned");
    command::process(command, &mut s)
}

#[cfg(feature = "wasm")]
pub fn export() -> Storage {
    STORAGE
        .get()
        .expect("storage not initialized")
        .read()
        .expect("storage lock poisoned")
        .export()
}
