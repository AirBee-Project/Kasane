use serde::Serialize;

use super::entity::UserSummary;
use super::privilege::PrivilegeRule;

#[derive(Debug, Serialize)]
pub struct UserInfoResponse {
    pub username: String,
    /// 現在有効な権限ルール。既に削除されたデータベース／テーブルを指すルールは
    /// 解決できないため一覧に現れない（実際の認可でも一致しない）。
    pub privileges: Vec<PrivilegeRule>,
}

/// `GET /users` のレスポンス。
///
/// 1 件ごとの権限は含めない。含めると 1 リクエストの読み取りが
/// 「利用者数 × 保持権限数」に比例してしまう。詳細は
/// `GET /users/{username}/privileges` で引く。
#[derive(Debug, Serialize)]
pub struct UserListResponse {
    pub users: Vec<UserSummary>,
    /// 続きがあるとき、次のリクエストの `after` に渡す利用者名。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}
