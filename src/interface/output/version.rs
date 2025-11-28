#[cfg(feature = "serde")]
use serde::Serialize;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct Version {
    pub version: String,
}
