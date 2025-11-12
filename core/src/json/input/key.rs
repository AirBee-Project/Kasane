use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ---------------------- Key管理 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateKey {
    pub space_name: String,
    pub key_name: String,
    pub key_type: KeyType,
    pub key_mode: KeyMode,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy, Encode, Decode, TS)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum KeyMode {
    UniqueKey = 0,
    MultiKey = 255,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Encode, Decode, TS)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum KeyType {
    Text(Vec<TextOption>),
    Float(Vec<FloatOption>),
    Int(Vec<IntOption>),
    Boolean(Vec<BooleanOption>),
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct DropKey {
    pub space_name: String,
    pub key_name: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy, Encode, Decode, TS)]
pub enum TextOption {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy, Encode, Decode, TS)]
pub enum FloatOption {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy, Encode, Decode, TS)]
pub enum IntOption {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy, Encode, Decode, TS)]
pub enum BooleanOption {}
