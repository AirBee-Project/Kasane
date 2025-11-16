use std::sync::Arc;

use crate::{
    command::tools::valid_name::valid_name,
    io::full::Storage,
    json::{input::DropUser, output::Output},
    user_error::UserError,
};

pub fn drop_user(v: DropUser, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
