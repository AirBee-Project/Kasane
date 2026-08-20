use crate::models::system::SystemInfoResponse;
use axum::Json;

/// サーバーのステータスとバージョン情報を取得する。
///
/// **必要な権限**: なし（認証のみ）
#[utoipa::path(
    get,
    path = "/system/info",
    responses(
        (status = 200, description = "Status and version retrieved successfully", body = SystemInfoResponse),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "System"
)]
#[tracing::instrument(skip_all)]
pub async fn get_system_info() -> Json<SystemInfoResponse> {
    Json(SystemInfoResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// ヘルスチェック
///
/// **必要な権限**: なし
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "System is healthy", body = String)
    ),
    tag = "System"
)]
#[tracing::instrument(skip_all)]
pub async fn health_check() -> &'static str {
    "OK"
}
