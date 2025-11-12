use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ---------------------- 権限管理 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct GrantDatabase {
    pub user_name: String,
    pub command: Vec<DatabaseCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, TS)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum DatabaseCommand {
    ALL = 0,
    CreateSpace = 1,
    DropSpace = 2,
    ShowSpaces = 3,
    Version = 4,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct GrantSpace {
    pub user_name: String,
    pub target_space: Vec<String>,
    pub command: Vec<SpaceCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, TS)]
#[serde(rename_all = "camelCase")]
pub enum SpaceCommand {
    ALL,
    CreateKey,
    DropKey,
    InfoSpace,
    ShowKeys,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct GrantKey {
    pub user_name: String,
    pub target_space: String,
    pub target_key: Vec<String>,
    pub command: Vec<KeyCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub enum KeyCommand {
    ALL,
    InsertValue,
    PatchValue,
    UpdateValue,
    DropKey,
    SelectValue,
    InfoKey,
    ShowValues,
    FilterValue,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct RevokeDatabase {
    pub user_name: String,
    pub command: Vec<DatabaseCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct RevokeSpace {
    pub user_name: String,
    pub target_space: Vec<String>,
    pub command: Vec<SpaceCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct RevokeKey {
    pub user_name: String,
    pub target_space: String,
    pub target_key: Vec<String>,
    pub command: Vec<KeyCommand>,
}
