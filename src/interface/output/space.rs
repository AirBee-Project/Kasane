#[cfg(feature = "serde")]
use serde::Serialize;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use super::key::InfoKey;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct ShowSpaces {
    pub space_names: Vec<String>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct InfoSpace {
    pub space_name: String,
    pub keys: Vec<InfoKey>,
}
