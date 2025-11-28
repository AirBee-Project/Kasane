use std::sync::Arc;

use crate::{
    io::full::Storage,
    interface::{input::DropKey, output::Output},
    user_error::UserError,
};

pub fn drop_key(v: DropKey, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
