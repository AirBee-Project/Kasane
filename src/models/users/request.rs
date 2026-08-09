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

/// `global` スコープに対する権限の設定リクエスト。制御面の `admin` を指定できる唯一の入口。
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetGlobalPrivilegeRequest {
    pub role: super::entity::UserRole,
}

/// データベース・テーブルスコープに対する権限の設定リクエスト。
/// ロールは [`DataRole`](super::entity::DataRole) なので `admin` は表現できない。
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetDataPrivilegeRequest {
    pub role: super::entity::DataRole,
}
