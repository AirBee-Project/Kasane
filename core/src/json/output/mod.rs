use std::collections::HashMap;

use kasane_logic::id::{SpaceTimeId, coordinates::Point};
use serde::Serialize;

use crate::{
    io::ValueEntry,
    json::input::{CommandDatabase, CommandKey, CommandSpace},
};

#[derive(Serialize)]
pub struct ShowSpaces {
    pub spacenames: Vec<String>,
}

#[derive(Serialize)]
pub struct Version {
    pub version: String,
}

#[derive(Serialize)]
pub struct InfoSpace {
    pub spacename: String,
    pub keynames: Vec<InfoKey>,
}

#[derive(Serialize)]
pub struct InfoKey {
    pub keyname: String,
    pub keytype: String,
    pub keymode: String,
}

#[derive(Serialize)]
pub struct Showkeys {
    pub keynames: Vec<String>,
}

#[derive(Serialize)]
pub struct Value {
    pub id: SpaceTimeId,
    pub center: Point,
    pub vertex: [Point; 8],
    pub id_string: String,
    pub value: Vec<(std::string::String, ValueEntry)>,
}
#[derive(Serialize)]
pub struct ShowUsers {
    pub users: Vec<String>,
}

#[derive(Serialize)]
pub struct InfoUser {
    pub user_name: String,
    // database_command: Vec<CommandDatabase>,
    // space_command: Vec<InfoUserSpace>,
    // key_commnad: Vec<InfoUserKey>,
}

#[derive(Serialize)]
pub struct InfoUserSpace {
    space_name: String,
    //space_commnad: Vec<CommandSpace>,
}

#[derive(Serialize)]
pub struct InfoUserKey {
    space_name: String,
    key_name: String,
    //space_commnad: Vec<CommandKey>,
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
