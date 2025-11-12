use serde::Serialize;

use super::key::InfoKey;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowSpaces {
    pub space_names: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoSpace {
    pub space_name: String,
    pub keys: Vec<InfoKey>,
}
