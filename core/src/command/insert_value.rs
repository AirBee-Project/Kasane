use std::sync::Arc;

use crate::{
    error::Error,
    io::{StorageTrait, full::Storage, tools::range::range},
    json::{input::InsertValue, output::Output},
};

pub fn insert_value(v: InsertValue, s: Arc<Storage>) -> Result<Output, Error> {
    todo!()
}
