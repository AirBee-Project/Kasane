use std::sync::Arc;

use crate::{
    io::full::Storage,
    json::{input::GrantKey, output::Output},
    user_error::UserError,
};

pub fn grant_key(_v: GrantKey, _s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
