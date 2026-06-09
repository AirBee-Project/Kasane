use serde::Serialize;
use utoipa::ToSchema;

use super::{domain::User, entity::UserRole};

#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfoResponse {
    pub username: String,
    pub is_global_admin: bool,
}

impl From<User> for UserInfoResponse {
    fn from(user: User) -> Self {
        Self {
            username: user.username,
            is_global_admin: user.is_global_admin,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PrivilegeInfoResponse {
    pub db_name: String,
    pub role: UserRole,
}
