use std::sync::Arc;

use crate::{
    io::full::Storage,
    json::{input::InsertValue, output::Output},
    user_error::UserError,
};

pub fn insert_value(v: InsertValue, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
