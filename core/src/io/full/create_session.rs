use crate::{io::full::Storage, user_error::UserError};

impl Storage {
    pub fn create_session(
        &self,
        session_id: &str,
        username: &str,
        expires_at: u64,
    ) -> Result<(), UserError> {
        Ok(())
    }
}
