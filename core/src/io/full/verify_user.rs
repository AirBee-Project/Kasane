use crate::{io::full::Storage, json::output::Output, location, user_error::UserError};

impl Storage {
    pub fn verify_user(&self, username: &str, password: &str) -> Result<(), UserError> {
        Ok(())
    }
}
