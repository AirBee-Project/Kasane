use std::sync::Arc;

use crate::{
    command::tools::valid_name::valid_name,
    io::{
        StorageTrait,
        full::{Storage, tools::password_hash::hash_password},
    },
    json::{input::CreateUser, output::Output},
    user_error::UserError,
};

pub fn create_user(v: CreateUser, s: Arc<Storage>) -> Result<Output, UserError> {
    //危険な入力がデータベースに侵入するのを防ぐ

    //エラーの位置
    let location = location!();

    //Userの名前のチェック
    match valid_name(&v.user_name) {
        Ok(_) => {}
        Err(e) => {
            return Err(UserError::UserNameVaildationError {
                name: v.user_name,
                reason: e,
                location: location,
            });
        }
    }

    //パスワードをHash化する
    let hashed = hash_password(&v.password).map_err(|err| UserError::UnKnown {
        message: err,
        location: location,
    })?;

    //ストレージに対して操作を実行する
    s.create_user(&v.user_name, hashed)
}
