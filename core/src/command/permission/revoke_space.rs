use std::sync::Arc;

use crate::{
    io::full::Storage,
    json::{input::RevokeSpace, output::Output},
    user_error::UserError,
};

pub fn revoke_space(_v: RevokeSpace, _s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
