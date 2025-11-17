// Module declarations and re-exports
pub mod info;
pub mod key;
pub mod permission;
pub mod range;
pub mod space;
pub mod user;
pub mod value;

// Re-export types from submodules
pub use info::*;
pub use key::*;
pub use permission::*;
pub use range::*;
pub use space::*;
pub use user::*;
pub use value::*;

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
    GrantKey(GrantKey),
    GrantUser(GrantUser),

    //権限取り上げ系
    RevokeDatabase(RevokeDatabase),
    RevokeSpace(RevokeSpace),
    RevokeKey(RevokeKey),
    RevokeUser(RevokeUser),
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "input/input.ts")]
pub struct Packet {
    pub command: Vec<Command>,
}
