use crate::interface::input::ValueEntry;
#[cfg(feature = "file")]
use crate::io::full::table_types::value_entry::ValueEntry;
use kasane_logic::space_time_id::SpaceTimeID;
#[cfg(feature = "serde")]
use serde::Serialize;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

/// A serializable representation of SpaceTimeID
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct SpaceTimeIDOutput {
    pub z: u8,
    pub f: [i64; 2],
    pub x: [u64; 2],
    pub y: [u64; 2],
}

impl From<SpaceTimeID> for SpaceTimeIDOutput {
    fn from(id: SpaceTimeID) -> Self {
        SpaceTimeIDOutput {
            z: id.z,
            f: id.f,
            x: id.x,
            y: id.y,
        }
    }
}

/// A value with its associated SpaceTimeID
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct Value {
    pub id: SpaceTimeIDOutput,
    pub value: ValueEntry,
}

/// Output for ShowValues command - returns all values for a single key
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct ShowValues {
    pub values: Vec<Value>,
}

/// Values for a single key in SelectValue response
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct KeyValues {
    pub key_name: String,
    pub values: Vec<Value>,
}

/// Output for SelectValue command - returns values for multiple keys within a range
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct SelectValue {
    pub key_values: Vec<KeyValues>,
}
