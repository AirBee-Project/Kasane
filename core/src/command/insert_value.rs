use std::sync::Arc;

use crate::{
    command::tools::valid_name::valid_name,
    io::full::Storage,
    json::{input::InsertValue, output::Output},
    user_error::UserError,
};

pub fn insert_value(v: InsertValue, s: Arc<Storage>) -> Result<Output, UserError> {
    //危険な入力がデータベースに侵入するのを防ぐ

    //Spaceの名前のチェック
    match valid_name(&v.space_name) {
        Ok(_) => {}
        Err(e) => {
            return Err(UserError::SpaceNameValidationError {
                name: v.space_name,
                reason: e,
                location: location!(),
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
                location: location!(),
            });
        }
    }

    todo!()
}
