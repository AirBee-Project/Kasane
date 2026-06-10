use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserMetadata {
    pub id: Uuid,
    pub password_hash: String,
    pub is_global_admin: bool,
    /// トークンの世代番号。
    ///
    /// パスワード変更や管理者権限の変更など、発行済みトークンを失効させたい
    /// 操作のたびにインクリメントする。JWT に埋め込んだ値と一致しないトークンは
    /// 無効として扱う。既存データ（このフィールドを持たない JSON）との互換性の
    /// ため `#[serde(default)]` で 0 として読み込む。
    #[serde(default)]
    pub token_version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[repr(u8)]
pub enum UserRole {
    Read = 1,
    Write = 2,
    Manage = 3,
}

impl UserRole {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(UserRole::Read),
            2 => Some(UserRole::Write),
            3 => Some(UserRole::Manage),
            _ => None,
        }
    }
}
