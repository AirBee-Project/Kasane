use crate::{io::full::Storage, user_error::UserError};

impl Storage {
    pub fn drop(&self, session_id: &str) -> Result<(), UserError> {
        todo!()
    }
}
