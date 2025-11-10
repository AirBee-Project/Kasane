use crate::{io::full::Storage, user_error::UserError};

impl Storage {
    pub fn cleanup_expired_sessions(&self) -> Result<(), UserError> {
        Ok(())
    }
}
