use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::entity::{UserMetadata, UserRole};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub id: Uuid,
    pub is_global_admin: bool,
    pub token_version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Privilege {
    pub username: String,
    pub db_name: String,
    pub role: UserRole,
}

impl User {
    pub fn from_meta(username: &str, meta: &UserMetadata) -> Self {
        Self {
            username: username.to_string(),
            id: meta.id,
            is_global_admin: meta.is_global_admin,
            token_version: meta.token_version,
        }
    }
}
