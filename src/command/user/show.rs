use std::sync::Arc;

use crate::{io::full::Storage, interface::output::Output, user_error::UserError};

pub fn show_users(s: Arc<Storage>) -> Result<Output, UserError> {
    s.show_users()
}
