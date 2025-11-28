use crate::io::io::Storage;

use crate::{
    interface::{input::SelectValue, output::Output},
    user_error::UserError,
};

pub fn select_value(v: SelectValue, s: &mut Storage) -> Result<Output, UserError> {
    s.select_value(v.key_names, v.range)
}
