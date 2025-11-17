use crate::interface::input::{DatabaseCommand, KeyCommand, SpaceCommand};
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ShowUsers {
    pub users: Vec<String>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InfoUser {
    pub user_name: String,
    pub database_command: Vec<DatabaseCommand>,
    pub space_command: Vec<InfoUserSpace>,
    pub key_commnad: Vec<InfoUserKey>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InfoUserSpace {
    pub space_name: String,
    pub space_commnad: Vec<SpaceCommand>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InfoUserKey {
    pub space_name: String,
    pub key_name: String,
    pub key_commnad: Vec<KeyCommand>,
}
