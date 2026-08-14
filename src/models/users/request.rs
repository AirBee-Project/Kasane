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

/// 1 ページで返す利用者数の既定値と上限。
pub const DEFAULT_USER_PAGE: usize = 100;
pub const MAX_USER_PAGE: usize = 1000;

/// `GET /users` のページング。利用者名の辞書順で進む。
#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
pub struct ListUsersQuery {
    /// この利用者名より後ろから返す（前ページの `next` をそのまま渡す）。
    pub after: Option<String>,
    /// 最大件数。既定 100、上限 1000。
    pub limit: Option<usize>,
}

impl ListUsersQuery {
    pub fn page_size(&self) -> usize {
        self.limit
            .unwrap_or(DEFAULT_USER_PAGE)
            .clamp(1, MAX_USER_PAGE)
    }
}
