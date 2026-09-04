use serde::Deserialize;

use super::privilege::PrivilegeRule;

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    /// 作成と同時に付与する権限。省略時は権限なし。
    #[serde(default)]
    pub privileges: Vec<PrivilegeRule>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePasswordRequest {
    pub password: String,
}

/// 1 ページで返す利用者数の既定値と上限。
pub const DEFAULT_USER_PAGE: usize = 100;
pub const MAX_USER_PAGE: usize = 1000;

/// `GET /users` のページング。利用者名の辞書順で進む。
#[derive(Debug, Default, Deserialize)]
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
