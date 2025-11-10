use crate::{location, user_error::UserError};
use std::str::Utf8Error;

impl From<Utf8Error> for UserError {
    fn from(err: Utf8Error) -> Self {
        let location = location!(); // 現在の位置を取得
        UserError::UnKnown {
            message: format!("UTF-8 conversion error: {}", err),
            location,
        }
    }
}
