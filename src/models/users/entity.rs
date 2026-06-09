use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserMetadata {
    pub id: Uuid,
    pub password_hash: String,
    pub is_global_admin: bool,
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
