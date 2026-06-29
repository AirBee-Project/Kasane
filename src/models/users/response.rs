use serde::Serialize;
use utoipa::ToSchema;

use super::{domain::User, entity::UserRole};

#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfoResponse {
    #[schema(example = "example_user")]
    pub username: String,
    #[schema(example = false)]
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
    #[schema(example = "example_database")]
    pub db_name: String,
    #[schema(example = "Write")]
    pub role: UserRole,
}
