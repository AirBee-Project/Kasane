use crate::models::system::SystemInfoResponse;
use axum::Json;

/// サーバーのステータスとバージョン情報を取得する。
///
/// ログイン済みのユーザーであれば誰でもアクセス可能です。
#[utoipa::path(
    get,
    path = "/system/info",
    responses(
        (status = 200, description = "Status and version retrieved successfully", body = SystemInfoResponse)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "System"
)]
pub async fn get_system_info() -> Json<SystemInfoResponse> {
    Json(SystemInfoResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
