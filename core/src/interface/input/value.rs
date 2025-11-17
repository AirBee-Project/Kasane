use crate::io::full::command_helpers::value_entry::ValueEntry;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::range::Range;

// ---------------------- Value管理 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct InsertValue {
    pub space_name: String,
    pub key_name: String,
    pub range: Range,
    pub value: ValueEntry,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct PatchValue {
    pub space_name: String,
    pub key_name: String,
    pub range: Range,
    pub value: ValueEntry,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateValue {
    pub space_name: String,
    pub key_name: String,
    pub range: Range,
    pub value: ValueEntry,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeleteValue {
    pub space_name: String,
    pub key_name: String,
    pub range: Range,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct SelectValue {
    pub space_name: String,
    pub key_names: Vec<String>,
    pub range: Range,
    //ここには出力物を加工できるような概念が必要
    pub vertex: bool,
    pub center: bool,
    pub id_string: bool,
    pub id_pure: bool,
}
