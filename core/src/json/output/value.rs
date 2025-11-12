use crate::io::full::tools::value_entry::ValueEntry;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Value {
    // pub id: SpaceTimeId,
    // pub center: Point,
    // pub vertex: [Point; 8],
    pub id_string: String,
    pub value: Vec<(std::string::String, ValueEntry)>,
}
