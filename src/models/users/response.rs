use serde::Serialize;
use utoipa::ToSchema;

use super::privilege::PrivilegeRule;

#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfoResponse {
    #[schema(example = "example_user")]
    pub username: String,
    /// 現在有効な権限ルール。既に削除されたデータベース／テーブルを指すルールは
    /// 解決できないため一覧に現れない（実際の認可でも一致しない）。
    pub privileges: Vec<PrivilegeRule>,
}

/// `GET /users/{username}/privileges` のレスポンス。
///
/// 個々のルールの追加・変更・削除は、対象ごとのサブリソース
/// （`.../privileges/global`、`.../privileges/databases/{db_name}` など）へ
/// `PUT` / `DELETE` する。
#[derive(Debug, Serialize, ToSchema)]
pub struct PrivilegesResponse {
    pub privileges: Vec<PrivilegeRule>,
}
