use std::sync::Arc;

use crate::{
    user_error::UserError,
    io::full::Storage,
    json::{input::DeleteValue, output::Output},
};

pub fn delete_value(v: DeleteValue, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
