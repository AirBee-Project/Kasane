use crate::io::io::Storage;

use crate::{
    interface::{input::Command, output::Output},
    user_error::UserError,
};
use std::sync::Arc;

pub mod key;
pub mod tools;
pub mod value;
pub mod version;

use key::{create_key, drop_key, info_key, show_keys};
use value::{delete_value, insert_value, patch_value, select_value, show_values, update_value};

//関数のディスパッチ関数
//関数の命令内容とストレージの参照権を関数に入力し、操作を行わせる
pub fn process(cmd: Command, s: &mut Storage) -> Result<Output, UserError> {
    match cmd {
        //データベース操作系
        // Command::CreateSpace(v) => create_space(v, s),
        // Command::DropSpace(v) => drop_space(v, s),
        // Command::ShowSpaces => show_spaces(s),
        // Command::InfoSpace(v) => info_space(v, s),
        // Command::Version => version(),

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
    }
}
