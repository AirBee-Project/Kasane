use std::sync::Arc;

use crate::{
    io::full::Storage,
    json::{input::GrantSpace, output::Output},
    user_error::UserError,
};

pub fn grant_space(v: GrantSpace, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
