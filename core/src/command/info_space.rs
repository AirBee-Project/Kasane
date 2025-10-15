use std::sync::Arc;

use crate::{
    command::tools::valid_name::valid_name,
    io::full::Storage,
    json::{input::InfoSpace, output::Output},
    user_error::UserError,
};

pub async fn info_space(v: InfoSpace, s: Arc<Storage>) -> Result<Output, UserError> {
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

    s.info_space(&v.space_name).await
}
