use std::sync::Arc;

use crate::{io::full::Storage, interface::output::Output, user_error::UserError};

pub fn show_spaces(s: Arc<Storage>) -> Result<Output, UserError> {
    //ストレージに対して操作を実行する

    s.show_spaces()
}
