use crate::{
    io::full::tools::value_entry::ValueEntry,
    json::{
        input::{AllOrChoose, CommandDatabase, KeyMode, KeyType},
        output::Output,
    },
    r#type::spacetimeid::SpaceTimeId,
    user_error::UserError,
};
use std::collections::HashMap;
pub mod full;

// ストレージに対する共通操作
// WasmやLMDBなど、多様なストレージ形態にはここで対応する
pub trait StorageTrait {
    //データベース操作系
    fn create_space(&self, space_name: &str) -> Result<Output, UserError>;
    fn drop_space(&self, space_name: &str) -> Result<Output, UserError>;
    fn info_space(&self, space_name: &str) -> Result<Output, UserError>;
    fn show_spaces(&self) -> Result<Output, UserError>;

    //key操作系
    fn create_key(
        &self,
        space_name: &str,
        key_name: &str,
        key_type: KeyType,
        key_mode: KeyMode,
    ) -> Result<Output, UserError>;
    fn drop_key(&self, space_name: &str, key_name: &str) -> Result<Output, UserError>;
    fn show_keys(&self, space_name: &str) -> Result<Output, UserError>;
    fn info_key(&self, space_name: &str, key_name: &str) -> Result<Output, UserError>;

    //Value操作系

    fn insert_value(
        &self,
        space_name: &str,
        key_name: &str,
        ids: Vec<SpaceTimeId>,
        value: ValueEntry,
    ) -> Result<Output, UserError>;
    fn patch_value(
        &self,
        space_name: &str,
        key_name: &str,
        ids: Vec<Vec<u8>>,
        value: ValueEntry,
    ) -> Result<Output, UserError>;
    fn update_value(
        &self,
        space_name: &str,
        key_name: &str,
        ids: Vec<Vec<u8>>,
        value: ValueEntry,
    ) -> Result<Output, UserError>;
    fn delete_value(
        &self,
        space_name: &str,
        key_name: &str,
        ids: Vec<Vec<u8>>,
    ) -> Result<Output, UserError>;
    fn select_value(
        &self,
        space_name: &str,
        key_name: Vec<String>,
        id: Vec<Vec<u8>>,
    ) -> Result<HashMap<Vec<u8>, Vec<(String, ValueEntry)>>, UserError>;
    fn show_values(
        &self,
        space_name: &str,
        key_name: &str,
    ) -> Result<HashMap<Vec<u8>, Vec<(String, ValueEntry)>>, UserError>;

    //ユーザー操作系
    fn create_user(&self, user_name: &str, password: &str) -> Result<Output, UserError>;
    fn drop_user(&self, user_name: &str) -> Result<Output, UserError>;
    fn info_user(&self, user_name: &str) -> Result<Output, UserError>;
    fn show_users(&self) -> Result<Output, UserError>;
    fn verify_user(&self, user_name: &str, password: &str) -> Result<bool, UserError>;

    //権限付与系
    fn grant_database(
        &self,
        user_name: &str,
        command: AllOrChoose<Vec<CommandDatabase>>,
    ) -> Result<Output, UserError>;
}
