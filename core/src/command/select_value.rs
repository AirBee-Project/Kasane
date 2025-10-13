use std::sync::Arc;

use crate::{
    error::Error,
    io::full::Storage,
    json::{input::SelectValue, output::Output},
};

pub fn select_value(v: SelectValue, s: Arc<Storage>) -> Result<Output, Error> {
    todo!()
}
