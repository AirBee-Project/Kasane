use serde::Serialize;

use crate::{
    io::full::tools::value_entry::ValueEntry,
    json::input::{DatabaseCommand, KeyCommand, SpaceCommand},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowSpaces {
    pub space_names: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoSpace {
    pub space_name: String,
    pub key_names: Vec<InfoKey>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoKey {
    pub key_name: String,
    pub key_type: String,
    pub key_mode: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Showkeys {
    pub key_names: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Value {
    // pub id: SpaceTimeId,
    // pub center: Point,
    // pub vertex: [Point; 8],
    pub id_string: String,
    pub value: Vec<(std::string::String, ValueEntry)>,
}
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Output {
    //CreateSpace,DropSpace,CreateKey,DropKey,InsertValue,UpdateValue,DeleteValue,CreateUser,DropUser,GrantDatabase,GrantSpacePrivilege,GrantKeyPrivilege,GrantToolPrivilege,RevokeDatabase,RevokeSpacePrivilege,RevokeKeyPrivilege,RevokeToolPrivilege
    Success,

    //データベース操作系
    InfoSpace(InfoSpace),
    ShowSpaces(ShowSpaces),
    Version(Version),

    //Key操作系
    Showkeys(Showkeys),
    InfoKey(InfoKey),

    //Value操作系
    SelectValue(Vec<Value>),
    ShowValues(Vec<Value>),

    //ユーザー操作系
    InfoUser(InfoUser),
    ShowUsers(ShowUsers),
}
