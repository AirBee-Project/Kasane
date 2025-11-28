use std::sync::Arc;

use crate::io::io::Storage;

use crate::{
    command::tools::valid_name::valid_name,
    interface::{input::InsertValue, output::Output},
    location,
    user_error::UserError,
};

pub fn insert_value(v: InsertValue, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
