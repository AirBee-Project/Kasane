use std::sync::Arc;

use crate::{
    io::full::Storage,
    json::{input::DeleteValue, output::Output},
    user_error::UserError,
};

pub fn delete_value(v: DeleteValue, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
