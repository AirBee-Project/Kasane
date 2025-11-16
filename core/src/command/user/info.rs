use std::sync::Arc;

use crate::{
    io::full::Storage,
    json::{input::InfoUser, output::Output},
    user_error::UserError,
};

pub fn info_user(v: InfoUser, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
