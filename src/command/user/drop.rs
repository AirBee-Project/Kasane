use std::sync::Arc;

use crate::{
    io::full::Storage,
    interface::{input::DropUser, output::Output},
    user_error::UserError,
};

pub fn drop_user(v: DropUser, s: Arc<Storage>) -> Result<Output, UserError> {
    s.drop_user(&v.user_name)
}
