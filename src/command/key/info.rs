use crate::io::io::Storage;

use crate::{
    command::tools::valid_name::valid_name,
    interface::{input::InfoKey, output::Output},
    location,
    user_error::UserError,
};
use std::sync::Arc;

pub fn info_key(v: InfoKey, s: Arc<Storage>) -> Result<Output, UserError> {
    match valid_name(&v.space_name) {
        Ok(_) => {}
        Err(e) => {
            return Err(UserError::SpaceNameValidationError {
                name: v.space_name,
                reason: e,
                location: location!(),
            });
        }
    }

    match valid_name(&v.key_name) {
        Ok(_) => {}
        Err(e) => {
            return Err(UserError::KeyNameValidationError {
                name: v.key_name,
                reason: e,
                location: location!(),
            });
        }
    }

    s.info_key(&v.space_name, &v.key_name)
}
