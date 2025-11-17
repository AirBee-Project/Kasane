use std::sync::Arc;

use crate::{
    command::tools::valid_name::valid_name,
    io::full::Storage,
    json::{input::InfoUser, output::Output},
    location,
    user_error::UserError,
};

pub fn info_user(v: InfoUser, s: Arc<Storage>) -> Result<Output, UserError> {
    // Validate user name
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

    s.info_user(&v.user_name)
}
