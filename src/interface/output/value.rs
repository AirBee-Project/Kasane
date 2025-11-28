#[cfg(feature = "file")]
use crate::io::full::table_types::value_entry::ValueEntry;
#[cfg(feature = "serde")]
use serde::Serialize;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct Value {
    // pub id: SpaceTimeId,
    pub id_string: String,
    #[cfg(feature = "file")]
    pub value: Vec<(std::string::String, ValueEntry)>,
}
