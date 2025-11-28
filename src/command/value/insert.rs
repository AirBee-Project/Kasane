use std::sync::Arc;

use crate::io::io::Storage;

use crate::{
    command::tools::valid_name::valid_name,
    interface::{input::InsertValue, output::Output},
    location,
    user_error::UserError,
};

pub fn insert_value(v: InsertValue, s: &mut Storage) -> Result<Output, UserError> {
    s.insert_value(v.key_name, v.range, v.value)
}
