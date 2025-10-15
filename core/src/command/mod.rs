use std::sync::Arc;

use crate::command::create_space::create_space;
use crate::command::show_spaces::show_spaces;
use crate::io::full::Storage;
use crate::{
    json::{input::Command, output::Output},
    user_error::UserError,
};
// pub mod create_key;
pub mod create_space;
// pub mod delete_value;
// pub mod drop_key;
// pub mod drop_space;
// pub mod drop_user;
// pub mod info_key;
// pub mod info_space;
// pub mod info_user;
// pub mod insert_value;
// pub mod patch_value;
// pub mod select_value;
// pub mod show_keys;
pub mod show_spaces;
// pub mod show_values;
pub mod tools;
// pub mod update_value;
// pub mod version;

//関数のディスパッチ関数
//関数の命令内容とストレージの参照権を関数に入力し、操作を行わせる
pub fn process(cmd: Command, s: Arc<Storage>) -> Result<Output, UserError> {
    match cmd {
        //データベース操作系
        Command::CreateSpace(v) => create_space(v, s),
        Command::DropSpace(v) => todo!(),
        Command::ShowSpaces => show_spaces(s),
        Command::InfoSpace(v) => todo!(),
        Command::Version => todo!(),

        //Key操作系
        Command::CreateKey(v) => todo!(),
        Command::DropKey(v) => todo!(),
        Command::ShowKeys(v) => todo!(),
        Command::InfoKey(v) => todo!(),

        //Value操作系
        Command::InsertValue(v) => todo!(),
        Command::PatchValue(v) => todo!(),
        Command::UpdateValue(v) => todo!(),
        Command::DeleteValue(v) => todo!(),
        Command::SelectValue(v) => todo!(),
        Command::ShowValues(v) => todo!(),
        //ツール系
        //Command::Transaction(v) => todo!(),

        // //ユーザー操作系
        // Command::CreateUser(v) => create_user(v, s),
        // Command::DropUser(v) => drop_user(v, s),
        // Command::InfoUser(v) => info_user(v, s),
        // Command::ShowUsers => show_users(s),

        // //権限付与系
        // Command::GrantDatabase(v) => grant_database(v, s),
        // Command::GrantSpace(v) => todo!(),
        // Command::GrantKeyPrivilege(v) => todo!(),

        // //権限取り上げる系
        // Command::RevokeDatabase(v) => todo!(),
        // Command::RevokeSpacePrivilege(v) => todo!(),
        // Command::RevokeKeyPrivilege(v) => todo!(),
    }
}
