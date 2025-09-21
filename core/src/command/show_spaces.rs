use std::sync::Arc;

use crate::{
    error::Error,
    io::{StorageTrait, full::Storage},
    json::output::Output,
};

pub fn show_spaces(s: Arc<Storage>) -> Result<Output, Error> {
    s.show_spaces()
}
