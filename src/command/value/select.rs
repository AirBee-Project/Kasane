use crate::io::io::Storage;

use crate::{
    interface::{input::SelectValue, output::Output},
    user_error::UserError,
};
use std::sync::Arc;

pub fn select_value(v: SelectValue, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
