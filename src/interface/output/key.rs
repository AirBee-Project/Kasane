#[cfg(feature = "serde")]
use serde::Serialize;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::interface::input::KeyType;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct Showkeys {
    pub key_names: Vec<Key>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct Key {
    pub key_name: String,
    pub key_type: KeyType,
}
