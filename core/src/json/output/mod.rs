// Module declarations and re-exports
pub mod space;
pub mod key;
pub mod value;
pub mod user;
pub mod version;

// Re-export types from submodules
pub use space::*;
pub use key::*;
pub use value::*;
pub use user::*;
pub use version::*;

use serde::Serialize;

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
