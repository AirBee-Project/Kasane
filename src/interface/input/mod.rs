// Module declarations and re-exports
pub mod key;
pub mod range;
pub mod space;
pub mod value;

// Re-export types from submodules
pub use key::*;
pub use range::*;
pub use value::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

// ---------------------- Packet & Command ----------------------

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub enum Command {
    //データベース操作系
    // CreateSpace(CreateSpace),
    // DropSpace(DropSpace),
    // InfoSpace(InfoSpace),
    // ShowSpaces,
    // Version,

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
    // CreateUser(CreateUser),
    // DropUser(DropUser),
    // InfoUser(InfoUser),
    // ShowUsers,
    // //権限付与系
    // GrantDatabase(GrantDatabase),
    // GrantSpace(GrantSpace),
    // GrantKey(GrantKey),
    // GrantUser(GrantUser),

    // //権限取り上げ系
    // RevokeDatabase(RevokeDatabase),
    // RevokeSpace(RevokeSpace),
    // RevokeKey(RevokeKey),
    // RevokeUser(RevokeUser),
}
