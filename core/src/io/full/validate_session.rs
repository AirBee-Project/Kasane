use crate::{io::full::Storage, user_error::UserError};

impl Storage {
    pub fn validate_session(&self, session_id: &str) -> Result<(), UserError> {
        Ok(())
    }
}
