use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ---------------------- Space管理 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpace {
    pub space_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct DropSpace {
    pub space_name: String,
}
