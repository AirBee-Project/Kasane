use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ---------------------- Key / Space情報 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct ShowKeys {
    pub space_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct InfoKey {
    pub space_name: String,
    pub key_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct InfoSpace {
    pub space_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct ShowValues {
    pub space_name: String,
    pub key_name: String,
}
