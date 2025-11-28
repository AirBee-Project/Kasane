use std::sync::Arc;

use crate::{
    io::full::Storage,
    interface::{input::InfoUser, output::Output},
    user_error::UserError,
};

pub fn info_user(v: InfoUser, s: Arc<Storage>) -> Result<Output, UserError> {
    s.info_user(&v.user_name)
}
