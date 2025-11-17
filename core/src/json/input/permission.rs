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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, TS, Encode, Decode)]
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, TS, Encode, Decode)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum SpaceCommand {
    ALL = 0,
    CreateKey = 1,
    DropKey = 2,
    InfoSpace = 3,
    ShowKeys = 4,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct GrantKey {
    pub user_name: String,
    pub target_space: String,
    pub target_key: Vec<String>,
    pub command: Vec<KeyCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, TS, Encode, Decode)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum KeyCommand {
    ALL = 0,
    InsertValue = 1,
    PatchValue = 2,
    UpdateValue = 3,
    DropKey = 4,
    SelectValue = 5,
    InfoKey = 6,
    ShowValues = 7,
    FilterValue = 8,
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

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct RevokeUser {
    pub user_name: String,
    pub command: Vec<UserCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct GrantUser {
    pub user_name: String,
    pub command: Vec<UserCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, TS, Encode, Decode)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum UserCommand {
    ALL = 0,
    CreateUser = 1,
    DropUser = 2,
    InfoUser = 3,
    ShowUsers = 4,
    GrantDatabase = 5,
    GrantSpace = 6,
    GrantKey = 7,
    GrauntUser = 8,
    RevokeDatabase = 9,
    RevokeSpace = 10,
    RevokeKey = 11,
    RevokeUser = 12,
}
