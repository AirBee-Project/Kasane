use crate::{io::full::Storage, user_error::UserError};

impl Storage {
    pub fn count_keepalive_sessions(&self) -> Result<usize, UserError> {
        Ok(0)
    }
}
