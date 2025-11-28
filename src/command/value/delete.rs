use std::sync::Arc;

use crate::io::io::Storage;

use crate::{
    interface::{input::DeleteValue, output::Output},
    user_error::UserError,
};

#[allow(unused_variables)]
pub fn delete_value(v: DeleteValue, s: &mut Storage) -> Result<Output, UserError> {
    s.delete_value(v.key_name, v.range)
}
