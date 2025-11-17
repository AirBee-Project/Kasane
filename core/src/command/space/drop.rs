use std::sync::Arc;

use crate::{
    command::tools::valid_name::valid_name,
    io::full::Storage,
    interface::{input::DropSpace, output::Output},
    user_error::UserError,
};

pub fn drop_space(v: DropSpace, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
