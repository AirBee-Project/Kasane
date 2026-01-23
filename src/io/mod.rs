use std::path::Path;

use crate::error::Error;
use crate::transaction::{read::ReadTxTrait, write::WriteTxTrait};

pub mod in_memory;
pub mod models;

pub mod on_disk;

pub type FieldId = u64;
pub type FlexRank = u64;
pub type RankId = u64;

///Kasaneの機能をTraitで抽象化し、複数のストレージに対応する。
pub trait Kasane {
    type WriteTx: WriteTxTrait;
    type ReadTx: ReadTxTrait;

    fn new(path: &Path) -> Result<Self, Error>
    where
        Self: Sized;

    fn write_begin(&mut self) -> Result<Self::WriteTx, Error>;
    fn read_begin(&self) -> Result<Self::ReadTx, Error>;
}
