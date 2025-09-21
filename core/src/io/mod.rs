use std::collections::{HashMap, HashSet};

use crate::{
    error::Error,
    json::{
        input::{KeyMode, KeyType},
        output::Output,
    },
};
use kasane_logic::id::SpaceTimeId;
use serde::{Deserialize, Serialize};
pub mod full;
pub mod tools;

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum ValueEntry {
    TEXT(String),
    BOOLEAN(bool),
    INT(i32),
    FLOAT(f32),
}

impl ValueEntry {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            ValueEntry::TEXT(s) => s.as_bytes().to_vec(),
            ValueEntry::BOOLEAN(b) => vec![*b as u8], // true=1, false=0
            ValueEntry::INT(i) => i.to_le_bytes().to_vec(), // i32 → 4バイト
            ValueEntry::FLOAT(f) => f.to_le_bytes().to_vec(), // f32 → 4バイト
        }
    }

    // 逆変換もあると便利
    pub fn from_bytes(keytype: KeyType, data: &[u8]) -> Option<Self> {
        match keytype {
            KeyType::TEXT => Some(ValueEntry::TEXT(String::from_utf8_lossy(data).to_string())),
            KeyType::BOOLEAN => Some(ValueEntry::BOOLEAN(data.get(0)? != &0)),
            KeyType::INT => {
                if data.len() != 4 {
                    return None;
                }
                let mut arr = [0u8; 4];
                arr.copy_from_slice(data);
                Some(ValueEntry::INT(i32::from_le_bytes(arr)))
            }
            KeyType::FLOAT => {
                if data.len() != 4 {
                    return None;
                }
                let mut arr = [0u8; 4];
                arr.copy_from_slice(data);
                Some(ValueEntry::FLOAT(f32::from_le_bytes(arr)))
            }
        }
    }
}
// StorageTrait は共通
pub trait StorageTrait {
    //データベース操作系
    fn create_space(&self, spacename: &str) -> Result<Output, Error>;
    fn drop_space(&self, spacename: &str) -> Result<Output, Error>;
    fn info_space(&self, spacename: &str) -> Result<Output, Error>;
    fn show_spaces(&self) -> Result<Output, Error>;

    //key操作系
    fn create_key(
        &self,
        spacename: &str,
        keyname: &str,
        keytype: KeyType,
        keymode: KeyMode,
    ) -> Result<Output, Error>;
    fn drop_key(&self, spacename: &str, keyname: &str) -> Result<Output, Error>;
    fn show_keys(&self, spacename: &str) -> Result<Output, Error>;
    fn info_key(&self, spacename: &str, keyname: &str) -> Result<Output, Error>;

    //Value操作系

    fn insert_value(
        &self,
        spacename: &str,
        keyname: &str,
        ids: Vec<Vec<u8>>,
        value: ValueEntry,
    ) -> Result<Output, Error>;
    fn patch_value(
        &self,
        spacename: &str,
        keyname: &str,
        ids: Vec<Vec<u8>>,
        value: ValueEntry,
    ) -> Result<Output, Error>;
    // fn update_value(
    //     &self,
    //     spacename: &str,
    //     keyname: &str,
    //     ids: Vec<Vec<u8>>,
    //     value: ValueEntry,
    // ) -> Result<Output, Error>;
    fn delete_value(
        &self,
        spacename: &str,
        keyname: &str,
        ids: Vec<Vec<u8>>,
    ) -> Result<Output, Error>;
    fn select_value(
        &self,
        spacename: &str,
        keyname: Vec<String>,
        id: Vec<Vec<u8>>,
    ) -> Result<HashMap<Vec<u8>, Vec<(String, ValueEntry)>>, Error>;
    fn show_values(
        &self,
        spacename: &str,
        keyname: &str,
    ) -> Result<HashMap<Vec<u8>, Vec<(String, ValueEntry)>>, Error>;

    //ユーザー操作系
    fn create_user(&self, username: &str, password: &str) -> Result<Output, Error>;
    fn drop_user(&self, username: &str) -> Result<Output, Error>;
    fn info_user(&self, username: &str) -> Result<Output, Error>;
    fn show_users(&self) -> Result<Output, Error>;
    fn verify_user(&self, username: &str, password: &str) -> Result<bool, Error>;
}
