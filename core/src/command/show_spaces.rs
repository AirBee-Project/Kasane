use std::sync::Arc;

use crate::{
    user_error::UserError,
    io::{StorageTrait, full::Storage},
    json::output::Output,
};

pub fn show_spaces(s: Arc<Storage>) -> Result<Output, UserError> {
    //ストレージに対して操作を実行する
    s.show_spaces()
}
