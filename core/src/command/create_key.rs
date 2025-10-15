use std::sync::Arc;

use crate::io::full::Storage;
use crate::json::input::CreateKey;
use crate::json::output::Output;
use crate::{command::tools::valid_name::valid_name, user_error::UserError};

pub fn create_key(v: CreateKey, s: Arc<Storage>) -> Result<Output, UserError> {
    //危険な入力がデータベースに侵入するのを防ぐ

    //エラーの位置
    let location = location!();

    //Spaceの名前のチェック
    match valid_name(&v.space_name) {
        Ok(_) => {}
        Err(e) => {
            return Err(UserError::SpaceNameValidationError {
                name: v.space_name,
                reason: e,
                location: location,
            });
        }
    }

    //Keyの名前のチェック
    match valid_name(&v.key_name) {
        Ok(_) => {}
        Err(e) => {
            return Err(UserError::KeyNameValidationError {
                name: v.key_name,
                reason: e,
                location: location,
            });
        }
    }

    //ストレージに対して操作を実行する
    s.create_key(&v.space_name, &v.key_name, v.key_type, v.key_mode)
}
