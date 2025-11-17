use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InfoKey {
    pub key_name: String,
    pub key_type: String,
    pub key_mode: String,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Showkeys {
    pub key_names: Vec<String>,
}
