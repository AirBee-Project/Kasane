use std::path::Path;

#[cfg(feature = "on_disk")]
use crate::error::Error;
use crate::transaction::{read::ReadTxTrait, write::WriteTxTrait};

pub mod on_disk;

pub trait Kasane {
    #[cfg(feature = "on_disk")]
    fn new(path: &Path) -> Result<Self, Error>
    where
        Self: Sized;

    #[cfg(feature = "in_memory")]
    fn new() -> Result<Self, Error>;

    fn write_begin(&'_ mut self) -> Result<impl WriteTxTrait, Error>;
    fn read_begin(&'_ self) -> Result<impl ReadTxTrait, Error>;
}
