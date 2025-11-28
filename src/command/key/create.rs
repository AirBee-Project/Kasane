use crate::interface::input::CreateKey;
use crate::interface::output::Output;
use crate::io::io::Storage;

use crate::location;
use crate::{command::tools::valid_name::valid_name, user_error::UserError};
use std::sync::Arc;

pub fn create_key(v: CreateKey, s: Arc<Storage>) -> Result<Output, UserError> {
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

    s.create_key(&v.space_name, &v.key_name, v.key_type, v.value_mode)
}
