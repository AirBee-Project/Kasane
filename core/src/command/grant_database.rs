use std::sync::Arc;

use crate::{
    io::full::Storage,
    json::{input::GrantDatabase, output::Output},
    user_error::UserError,
};

pub fn grant_database(v: GrantDatabase, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
