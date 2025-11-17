use std::collections::HashSet;

use crate::json::input::KeyCommand;

impl KeyCommand {
    pub fn all() -> HashSet<KeyCommand> {
        let mut set = HashSet::new();
        set.insert(KeyCommand::InsertValue);
        set.insert(KeyCommand::PatchValue);
        set.insert(KeyCommand::UpdateValue);
        set.insert(KeyCommand::DropKey);
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
            InfoSpace = 5,
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
            ShowKeys = 3,
            InfoKey = 4,
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
            SelectValue = 4,
            DeleteValue = 5,
            ShowValues = 6,
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

        set.insert(KeyCommand::SelectValue);
        set.insert(KeyCommand::InfoKey);
        set.insert(KeyCommand::ShowValues);
        set.insert(KeyCommand::FilterValue);
        set
    }
}
