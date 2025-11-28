use crate::interface::input::{DatabaseCommand, KeyCommand, SpaceCommand};
#[cfg(feature = "serde")]
use serde::Serialize;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct ShowUsers {
    pub users: Vec<String>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct InfoUser {
    pub user_name: String,
    pub database_command: Vec<DatabaseCommand>,
    pub space_command: Vec<InfoUserSpace>,
    pub key_commnad: Vec<InfoUserKey>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct InfoUserSpace {
    pub space_name: String,
    pub space_commnad: Vec<SpaceCommand>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct InfoUserKey {
    pub space_name: String,
    pub key_name: String,
    pub key_commnad: Vec<KeyCommand>,
}
