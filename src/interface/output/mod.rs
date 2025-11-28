// Module declarations and re-exports
pub mod key;
pub mod space;
pub mod user;
pub mod value;
pub mod version;

// Re-export types from submodules
pub use key::*;
pub use space::*;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;
pub use user::*;
pub use value::*;
pub use version::*;

#[cfg(feature = "serde")]
use serde::Serialize;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "output/output.ts"))]
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
