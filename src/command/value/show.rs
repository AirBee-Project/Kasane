use crate::io::io::Storage;

use crate::{
    interface::{input::ShowValues, output::Output},
    user_error::UserError,
};
use std::sync::Arc;

pub fn show_values(v: ShowValues, s: &mut Storage) -> Result<Output, UserError> {
    s.show_values(v.key_name)
}
