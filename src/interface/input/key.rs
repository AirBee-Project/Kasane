#[cfg(feature = "file")]
use bincode::{Decode, Encode};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

// ---------------------- Key管理 ----------------------

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct CreateKey {
    // pub space_name: String,
    pub key_name: String,
    pub key_type: KeyType,
}

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "file", derive(Encode, Decode))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[repr(u8)]
pub enum KeyType {
    Text,
    Float,
    Int,
    Boolean,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct DropKey {
    // pub space_name: String,
    pub key_name: String,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct ShowKeys {
    // pub space_name: String,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct InfoKey {
    // pub space_name: String,
    pub key_name: String,
    pub key_type: KeyType,
}
