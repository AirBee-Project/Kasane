use crate::json::input::{DatabaseCommand, KeyCommand, SpaceCommand};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowUsers {
    pub users: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoUser {
    pub user_name: String,
    database_command: Vec<DatabaseCommand>,
    space_command: Vec<InfoUserSpace>,
    key_commnad: Vec<InfoUserKey>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoUserSpace {
    space_name: String,
    space_commnad: Vec<SpaceCommand>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoUserKey {
    space_name: String,
    key_name: String,
    key_commnad: Vec<KeyCommand>,
}
