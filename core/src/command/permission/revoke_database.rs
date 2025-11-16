use std::sync::Arc;

use crate::{
    io::full::Storage,
    json::{input::RevokeDatabase, output::Output},
    user_error::UserError,
};

pub fn revoke_database(_v: RevokeDatabase, _s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
