// Module declarations and re-exports
pub mod space;
pub mod key;
pub mod value;
pub mod range;
pub mod info;
pub mod user;
pub mod permission;

// Re-export types from submodules
pub use space::*;
pub use key::*;
pub use value::*;
pub use range::*;
pub use info::*;
pub use user::*;
pub use permission::*;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ---------------------- Packet & Command ----------------------

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub enum Command {
    //データベース操作系
    CreateSpace(CreateSpace),
    DropSpace(DropSpace),
    InfoSpace(InfoSpace),
    ShowSpaces,
    Version,

    //Key操作系
    CreateKey(CreateKey),
    DropKey(DropKey),
    ShowKeys(ShowKeys),
    InfoKey(InfoKey),

    //Value操作系
    InsertValue(InsertValue),
    PatchValue(PatchValue),
    UpdateValue(UpdateValue),
    DeleteValue(DeleteValue),
    SelectValue(SelectValue),
    ShowValues(ShowValues),
    //ツール系
    //Transaction(Vec<Command>),

    //ユーザー操作系
    CreateUser(CreateUser),
    DropUser(DropUser),
    InfoUser(InfoUser),
    ShowUsers,

    //権限付与系
    GrantDatabase(GrantDatabase),
    GrantSpace(GrantSpace),
    GrantKeyPrivilege(GrantKey),

    //権限取り上げ系
    RevokeDatabase(RevokeDatabase),
    RevokeSpacePrivilege(RevokeSpace),
    RevokeKeyPrivilege(RevokeKey),
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "input/input.ts")]
pub struct Packet {
    pub command: Vec<Command>,
}
