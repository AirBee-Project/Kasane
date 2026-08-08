use serde::Deserialize;
use utoipa::ToSchema;

use super::privilege::PrivilegeRule;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    #[schema(example = "example_user")]
    pub username: String,
    #[schema(example = "secret123")]
    pub password: String,
    /// 作成と同時に付与する権限。省略時は権限なし。
    #[serde(default)]
    pub privileges: Vec<PrivilegeRule>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdatePasswordRequest {
    #[schema(example = "new_secret123")]
    pub password: String,
}

/// 1 つの対象に対する権限の設定リクエスト。対象はパスで指定する。
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetPrivilegeRequest {
    pub role: super::entity::UserRole,
}
