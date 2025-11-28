use crate::io::io::Storage;

use crate::{
    interface::{input::DropKey, output::Output},
    user_error::UserError,
};
use std::sync::Arc;

pub fn drop_key(v: DropKey, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
