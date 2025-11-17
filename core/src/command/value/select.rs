use std::sync::Arc;

use crate::{
    user_error::UserError,
    io::full::Storage,
    interface::{input::SelectValue, output::Output},
};

pub fn select_value(v: SelectValue, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
