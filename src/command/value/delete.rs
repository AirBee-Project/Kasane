use std::sync::Arc;

use crate::io::io::Storage;

use crate::{
    interface::{input::DeleteValue, output::Output},
    user_error::UserError,
};

#[allow(unused_variables)]
pub fn delete_value(v: DeleteValue, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
