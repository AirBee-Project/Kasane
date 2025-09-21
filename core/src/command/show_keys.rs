use std::sync::Arc;

use crate::{
    error::Error,
    io::{StorageTrait, full::Storage},
    json::{input::ShowKeys, output::Output},
};

pub fn show_keys(v: ShowKeys, s: Arc<Storage>) -> Result<Output, Error> {
    s.show_keys(&v.space_name)
}
