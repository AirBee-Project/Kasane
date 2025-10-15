use crate::{io::full::Storage, json::output::Output, user_error::UserError};

enum CreateUserTxError {
    UserAlreadyExists,
}

impl Storage {
    pub fn verify_user(&self, user_name: &str, password: &str) -> Result<Output, UserError> {
        let location = location!();
        if user_name != "admin" {
            return Err(UserError::UnKnown {
                message: "user name is admin".to_owned(),
                location: location,
            });
        }

        if password != "admin" {
            return Err(UserError::UnKnown {
                message: "user name is admin".to_owned(),
                location: location,
            });
        }

        Ok(Output::Success)
    }
}
