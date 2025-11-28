use crate::io::full::table_types::value_entry::ValueEntry;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Value {
    // pub id: SpaceTimeId,
    pub id_string: String,
    pub value: Vec<(std::string::String, ValueEntry)>,
}
