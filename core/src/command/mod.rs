use std::sync::Arc;
#[cfg(feature = "wasm")]
use std::sync::Arc;
#[cfg(feature = "full")]
use std::sync::{Arc, Mutex};

use rand::prelude;

use crate::command::create_key::create_key;
use crate::command::create_space::create_space;
use crate::command::create_user::create_user;
use crate::command::delete_value::delete_value;
use crate::command::drop_key::drop_key;
use crate::command::drop_space::drop_space;
use crate::command::drop_user::drop_user;
use crate::command::info_key::info_key;
use crate::command::info_space::info_space;
use crate::command::info_user::info_user;
use crate::command::insert_value::insert_value;
use crate::command::patch_value::patch_value;
use crate::command::select_value::select_value;
use crate::command::show_keys::show_keys;
use crate::command::show_spaces::show_spaces;
use crate::command::show_user::show_users;
//use crate::command::update_value::update_value;

use crate::command::show_values::show_values;
use crate::io::full::Storage;
use crate::{
    command::version::version,
    error::Error,
    json::{input::Command, output::Output},
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
pub mod show_user;
pub mod show_values;
pub mod tools;
pub mod triangle;
//pub mod update_value;
pub mod version;

//関数のディスパッチ関数
//関数の命令内容とストレージの参照権を関数に入力し、操作を行わせる
pub fn process(cmd: Command, s: Arc<Storage>) -> Result<Output, Error> {
    match cmd {
        //データベース操作系
        Command::CreateSpace(v) => create_space(v, s),
        Command::DropSpace(v) => drop_space(v, s),
        Command::ShowSpaces => show_spaces(s),
        Command::InfoSpace(v) => info_space(v, s),
        Command::Version => version(),

        //Key操作系
        Command::CreateKey(v) => create_key(v, s),
        Command::DropKey(v) => drop_key(v, s),
        Command::ShowKeys(v) => show_keys(v, s),
        Command::InfoKey(v) => info_key(v, s),

        //Value操作系
        Command::InsertValue(v) => insert_value(v, s),
        Command::PatchValue(v) => patch_value(v, s),
        //Command::UpdateValue(v) => update_value(v, s),
        Command::DeleteValue(v) => delete_value(v, s),
        Command::SelectValue(v) => select_value(v, s),
        Command::ShowValues(v) => show_values(v, s),

        //ツール系
        //Command::Transaction(v) => todo!(),

        //ユーザー操作系
        Command::CreateUser(v) => create_user(v, s),
        Command::DropUser(v) => drop_user(v, s),
        Command::InfoUser(v) => info_user(v, s),
        Command::ShowUsers => show_users(s),
        //権限付与系
        // Command::GrantDatabase(v) => todo!(),
        // Command::GrantSpacePrivilege(v) => todo!(),
        // Command::GrantKeyPrivilege(v) => todo!(),

        // //権限取り上げる系
        // Command::RevokeDatabase(v) => todo!(),
        // Command::RevokeSpacePrivilege(v) => todo!(),
        // Command::RevokeKeyPrivilege(v) => todo!(),
    }
}
