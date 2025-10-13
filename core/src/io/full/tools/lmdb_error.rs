use std::error::Error;

use crate::user_error::UserError;
use lmdb::Error as LmdbError;

impl From<LmdbError> for UserError {
    fn from(err: LmdbError) -> Self {
        let location = location!(); // マクロで現在の位置を取得
        match err {
            LmdbError::KeyExist => UserError::UnKnown {
                message: "The key already exists in LMDB".to_string(),
                location: location,
            }, // 特定の場合は別のエラーに置き換え可能
            LmdbError::NotFound => UserError::UnKnown {
                message: "The key was not found in LMDB".to_string(),
                location: location,
            },
            _ => UserError::LmdbError {
                message: err.description().to_string(),
                location,
            },
        }
    }
}
