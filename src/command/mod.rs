#[cfg(feature = "file")]
use std::sync::Arc;

#[cfg(feature = "file")]
use crate::io::full::Storage;
#[cfg(feature = "file")]
use crate::{
    interface::{input::Command, output::Output},
    user_error::UserError,
};

#[cfg(feature = "file")]
pub mod key;
// pub mod permission;
#[cfg(feature = "file")]
pub mod space;
pub mod tools;
#[cfg(feature = "file")]
pub mod user;
#[cfg(feature = "file")]
pub mod value;
#[cfg(feature = "file")]
pub mod version;

#[cfg(feature = "file")]
use key::{create_key, drop_key, info_key, show_keys};
// use permission::{
//     grant_database, grant_key, grant_space, grant_user, revoke_database, revoke_key, revoke_space,
//     revoke_user,
// };
#[cfg(feature = "file")]
use space::{create_space, drop_space, info_space, show_spaces};
#[cfg(feature = "file")]
use user::{create_user, show_users};
#[cfg(feature = "file")]
use value::{delete_value, insert_value, patch_value, select_value, show_values, update_value};
#[cfg(feature = "file")]
use version::version;

//関数のディスパッチ関数
//関数の命令内容とストレージの参照権を関数に入力し、操作を行わせる
#[cfg(feature = "file")]
pub async fn process(cmd: Command, s: Arc<&Storage>) -> Result<Output, UserError> {
    match cmd {
        //データベース操作系
        Command::CreateSpace(v) => create_space(v, s),
        Command::DropSpace(v) => drop_space(v, s),
        Command::ShowSpaces => show_spaces(s),
        Command::InfoSpace(v) => info_space(v, s),
        Command::Version => version(s),

        //Key操作系
        Command::CreateKey(v) => create_key(v, s),
        Command::DropKey(v) => drop_key(v, s),
        Command::ShowKeys(v) => show_keys(v, s),
        Command::InfoKey(v) => info_key(v, s),

        //Value操作系
        Command::InsertValue(v) => insert_value(v, s),
        Command::PatchValue(v) => patch_value(v, s),
        Command::UpdateValue(v) => update_value(v, s),
        Command::DeleteValue(v) => delete_value(v, s),
        Command::SelectValue(v) => select_value(v, s),
        Command::ShowValues(v) => show_values(v, s),
        //ツール系
        //Command::Transaction(v) => todo!(),

        //ユーザー操作系
        // Command::CreateUser(v) => create_user(v, s),
        // Command::DropUser(v) => drop_user(v, s),
        // Command::InfoUser(v) => info_user(v, s),
        // Command::ShowUsers => show_users(s),
        //権限付与系
        // Command::GrantDatabase(v) => grant_database(v, s),
        // Command::GrantSpace(v) => grant_space(v, s),
        // Command::GrantKey(v) => grant_key(v, s),
        // Command::GrantUser(v) => grant_user(v, s),

        // //権限取り上げる系
        // Command::RevokeDatabase(v) => revoke_database(v, s),
        // Command::RevokeSpace(v) => revoke_space(v, s),
        // Command::RevokeKey(v) => revoke_key(v, s),
        // Command::RevokeUser(v) => revoke_user(v, s),
    }
}
