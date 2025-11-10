use std::sync::Arc;

use argon2::password_hash::Output;

use crate::{
    command::tools::valid_name::valid_name, io::full::Storage, json::input::CreateUser, location,
    user_error::UserError,
};

pub fn create_user(v: CreateUser, s: Arc<Storage>) -> Result<Output, UserError> {
    match valid_name(&v.user_name) {
        Ok(_) => {}
        Err(e) => {
            return Err(UserError::UserNameVaildationError {
                name: v.user_name,
                reason: e,
                location: location!(),
            });
        }
    }
}
