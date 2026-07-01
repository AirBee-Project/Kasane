use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct SystemInfoResponse {
    /// サーバーのステータス（通常は "ok"）
    pub status: String,
    /// サーバーのバージョン
    pub version: String,
}
