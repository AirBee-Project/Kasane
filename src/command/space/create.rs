use std::sync::Arc;

use crate::{
    command::tools::valid_name::valid_name,
    io::full::Storage,
    interface::{input::CreateSpace, output::Output},
    location,
    user_error::UserError,
};

pub fn create_space(v: CreateSpace, s: Arc<Storage>) -> Result<Output, UserError> {
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

    //ストレージに対して操作を実行する
    s.create_space(&v.space_name)
}
