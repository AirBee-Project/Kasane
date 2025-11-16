use serde::Serialize;
use ts_rs::TS;

use super::key::InfoKey;

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ShowSpaces {
    pub space_names: Vec<String>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InfoSpace {
    pub space_name: String,
    pub keys: Vec<InfoKey>,
}
