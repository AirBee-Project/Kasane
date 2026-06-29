use serde::Deserialize;
use utoipa::ToSchema;

use super::entity::UserRole;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    #[schema(example = "example_user")]
    pub username: String,
    #[schema(example = "secret123")]
    pub password: String,
    #[schema(example = false)]
    pub is_global_admin: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdatePasswordRequest {
    #[schema(example = "newsecret123")]
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdatePrivilegeRequest {
    #[schema(example = "Write")]
    pub role: UserRole,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAdminRequest {
    #[schema(example = false)]
    pub is_global_admin: bool,
}
