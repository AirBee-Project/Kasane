use std::sync::Arc;

use crate::{
    io::full::Storage,
    json::{input::SelectValue, output::Output},
    user_error::UserError,
};

pub fn show_users(s: Arc<Storage>) -> Result<Output, UserError> {
    s.show_users()
}
