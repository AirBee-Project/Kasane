use std::{collections::HashSet, sync::Arc};

use crate::{
    command::tools::valid_name::valid_name,
    io::{StorageTrait, full::Storage},
    json::{
        input::{AllOrChoose, CommandDatabase, GrantDatabase},
        output::Output,
    },
    user_error::UserError,
};

use crate::json::input::AllOrChoose::{All, Choose};

pub fn grant_database(v: GrantDatabase, s: Arc<Storage>) -> Result<Output, UserError> {
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

    //コマンドの重複排除
    let command = match v.command {
        Choose(v) => {
            let unique: HashSet<CommandDatabase> = v.into_iter().collect();
            AllOrChoose::Choose(Vec::from_iter(unique))
        }
        All => AllOrChoose::All,
    };

    //ストレージに対して操作を実行する
    s.grant_database(&v.user_name, command)
}
