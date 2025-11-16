use std::sync::Arc;

use crate::{
    io::full::Storage,
    json::{input::RevokeKey, output::Output},
    user_error::UserError,
};

pub fn revoke_key(_v: RevokeKey, _s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
