use std::sync::Arc;

use crate::{
    command::tools::valid_name::valid_name,
    user_error::UserError,
    io::{StorageTrait, full::Storage},
    json::{input::InfoKey, output::Output},
};

pub fn info_key(v: InfoKey, s: Arc<Storage>) -> Result<Output, UserError> {
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

    s.info_key(&v.space_name, &v.key_name)
}
