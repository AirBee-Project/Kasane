#[cfg(feature = "file")]
use std::sync::Arc;

use crate::{
    command::tools::valid_name::valid_name,
    interface::{input::ShowKeys, output::Output},
    location,
    user_error::UserError,
};
#[cfg(feature = "file")]
use crate::io::full::Storage;

#[cfg(feature = "file")]
pub fn show_keys(v: ShowKeys, s: Arc<&Storage>) -> Result<Output, UserError> {
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

    //ストレージに対して操作を実行する
    s.show_keys(&v.space_name)
}
