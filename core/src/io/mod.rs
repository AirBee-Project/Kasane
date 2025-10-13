use std::collections::HashMap;

use crate::{
    io::value_entry::ValueEntry,
    json::{
        input::{KeyMode, KeyType},
        output::Output,
    },
    user_error::UserError,
};
pub mod full;
pub mod tools;
pub mod value_entry;

// ストレージに対する共通操作
// WasmやLMDBなど、多様なストレージ形態にはここで対応する
pub trait StorageTrait {
    //データベース操作系
    fn create_space(&self, spacename: &str) -> Result<Output, UserError>;
    fn drop_space(&self, spacename: &str) -> Result<Output, UserError>;
    fn info_space(&self, spacename: &str) -> Result<Output, UserError>;
    fn show_spaces(&self) -> Result<Output, UserError>;

    //key操作系
    fn create_key(
        &self,
        spacename: &str,
        keyname: &str,
        keytype: KeyType,
        keymode: KeyMode,
    ) -> Result<Output, UserError>;
    fn drop_key(&self, spacename: &str, keyname: &str) -> Result<Output, UserError>;
    fn show_keys(&self, spacename: &str) -> Result<Output, UserError>;
    fn info_key(&self, spacename: &str, keyname: &str) -> Result<Output, UserError>;

    //Value操作系

    fn insert_value(
        &self,
        spacename: &str,
        keyname: &str,
        ids: Vec<Vec<u8>>,
        value: ValueEntry,
    ) -> Result<Output, UserError>;
    fn patch_value(
        &self,
        spacename: &str,
        keyname: &str,
        ids: Vec<Vec<u8>>,
        value: ValueEntry,
    ) -> Result<Output, UserError>;
    fn update_value(
        &self,
        spacename: &str,
        keyname: &str,
        ids: Vec<Vec<u8>>,
        value: ValueEntry,
    ) -> Result<Output, UserError>;
    fn delete_value(
        &self,
        spacename: &str,
        keyname: &str,
        ids: Vec<Vec<u8>>,
    ) -> Result<Output, UserError>;
    fn select_value(
        &self,
        spacename: &str,
        keyname: Vec<String>,
        id: Vec<Vec<u8>>,
    ) -> Result<HashMap<Vec<u8>, Vec<(String, ValueEntry)>>, UserError>;
    fn show_values(
        &self,
        spacename: &str,
        keyname: &str,
    ) -> Result<HashMap<Vec<u8>, Vec<(String, ValueEntry)>>, UserError>;

    //ユーザー操作系
    fn create_user(&self, username: &str, password: &str) -> Result<Output, UserError>;
    fn drop_user(&self, username: &str) -> Result<Output, UserError>;
    fn info_user(&self, username: &str) -> Result<Output, UserError>;
    fn show_users(&self) -> Result<Output, UserError>;
    fn verify_user(&self, username: &str, password: &str) -> Result<bool, UserError>;
}
