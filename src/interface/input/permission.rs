#[cfg(feature = "file")]
use bincode::{Decode, Encode};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

// ---------------------- スコープ ----------------------

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub enum Scope {
    Global,
    Database,
    Space(Vec<String>),
    Key { space: String, keys: Vec<String> },
    User,
}

// ---------------------- コマンド列挙 ----------------------

// ========== Database Commands ==========
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "file", derive(Encode, Decode))]
#[repr(u8)]
pub enum DatabaseCommand {
    ALL = 0,
    CreateSpace = 1,
    DropSpace = 2,
    ShowSpaces = 3,
    Version = 4,
    InfoSpace = 5,
}

// ========== Space Commands ==========
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "file", derive(Encode, Decode))]
#[repr(u8)]
pub enum SpaceCommand {
    ALL = 0,
    CreateKey = 1,
    DropKey = 2,
    ShowKeys = 3,
    InfoKey = 4,
}

// ========== Key Commands ==========
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "file", derive(Encode, Decode))]
#[repr(u8)]
pub enum KeyCommand {
    ALL = 0,
    InsertValue = 1,
    PatchValue = 2,
    UpdateValue = 3,
    SelectValue = 4,
    DeleteValue = 5,
    ShowValues = 6,
}

// ========== User Commands ==========
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "file", derive(Encode, Decode))]
#[repr(u8)]
pub enum UserCommand {
    ALL = 0,

    // User management
    CreateUser = 1,
    DropUser = 2,
    InfoUser = 3,
    ShowUsers = 4,

    // Grant
    GrantUser = 5,
    GrantDatabase = 6,
    GrantSpace = 7,
    GrantKey = 8,

    // Revoke
    RevokeUser = 9,
    RevokeDatabase = 10,
    RevokeSpace = 11,
    RevokeKey = 12,
}

// ---------------------- 権限操作（Grant / Revoke） ----------------------

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub enum PermissionCommand {
    Database(Vec<DatabaseCommand>),
    Space(Vec<SpaceCommand>),
    Key(Vec<KeyCommand>),
    User(Vec<UserCommand>),
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct Grant {
    pub user_name: String,
    pub scope: Scope,
    pub command: PermissionCommand,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct Revoke {
    pub user_name: String,
    pub scope: Scope,
    pub command: PermissionCommand,
}
