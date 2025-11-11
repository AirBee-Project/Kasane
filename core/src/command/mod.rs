use std::sync::Arc;

use crate::command::create_key::create_key;
use crate::command::create_space::create_space;
use crate::command::info_key::info_key;
use crate::command::info_space::info_space;
use crate::command::info_user::info_user;
use crate::command::insert_value::insert_value;
use crate::command::show_keys::show_keys;
use crate::command::show_spaces::show_spaces;
use crate::command::version::version;
use crate::io::full::Storage;
use crate::{
    json::{input::Command, output::Output},
    user_error::UserError,
};
pub mod create_key;
pub mod create_space;
pub mod create_user;
pub mod delete_value;
pub mod drop_key;
pub mod drop_space;
pub mod drop_user;
pub mod info_key;
pub mod info_space;
pub mod info_user;
pub mod insert_value;
pub mod patch_value;
pub mod select_value;
pub mod show_keys;
pub mod show_spaces;
pub mod show_values;
pub mod tools;
pub mod update_value;
pub mod version;

//関数のディスパッチ関数
//関数の命令内容とストレージの参照権を関数に入力し、操作を行わせる
pub async fn process(cmd: Command, s: Arc<Storage>) -> Result<Output, UserError> {
    match cmd {
        //データベース操作系
        Command::CreateSpace(v) => create_space(v, s),
        Command::DropSpace(v) => todo!(),
        Command::ShowSpaces => show_spaces(s),
        Command::InfoSpace(v) => info_space(v, s),
        Command::Version => version(s),

        //Key操作系
        Command::CreateKey(v) => create_key(v, s),
        Command::DropKey(v) => todo!(),
        Command::ShowKeys(v) => show_keys(v, s),
        Command::InfoKey(v) => info_key(v, s),

        //Value操作系
        Command::InsertValue(v) => insert_value(v, s),
        Command::PatchValue(v) => todo!(),
        Command::UpdateValue(v) => todo!(),
        Command::DeleteValue(v) => todo!(),
        Command::SelectValue(v) => todo!(),
        Command::ShowValues(v) => todo!(),

        //ツール系
        //Command::Transaction(v) => todo!(),

        //ユーザー操作系
        Command::CreateUser(v) => todo!(),
        Command::DropUser(v) => todo!(),
        Command::InfoUser(v) => info_user(v, s),
        Command::ShowUsers => todo!(),

        //権限付与系
        Command::GrantDatabase(v) => todo!(),
        Command::GrantSpace(v) => todo!(),
        Command::GrantKeyPrivilege(v) => todo!(),

        //権限取り上げる系
        Command::RevokeDatabase(v) => todo!(),
        Command::RevokeSpacePrivilege(v) => todo!(),
        Command::RevokeKeyPrivilege(v) => todo!(),
    }
}
