use std::sync::Arc;

use crate::{
    io::full::Storage,
    json::{input::GrantDatabase, output::Output},
    user_error::UserError,
};

pub fn grant_database(_v: GrantDatabase, _s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
