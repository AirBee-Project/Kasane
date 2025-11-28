use std::sync::{Arc, Mutex};

use crate::{
    command::tools::valid_name::valid_name,
    interface::{input::CreateSpace, output::Output},
    io::io::Storage,
    location,
    user_error::UserError,
};

pub fn create_space(v: CreateSpace, s: &mut Storage) -> Result<Output, UserError> {
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

    s.create_space(v.space_name)
}
