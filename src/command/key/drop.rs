use crate::io::io::Storage;

use crate::{
    interface::{input::DropKey, output::Output},
    user_error::UserError,
};
use std::sync::Arc;

pub fn drop_key(v: DropKey, s: &mut Storage) -> Result<Output, UserError> {
    s.drop_key(v.key_name)
}
