use crate::middleware::auth::AuthUser;
use axum::Extension;
use axum::{
    Json,
    extract::{Path, State},
};

use crate::{
    AppState, error::AppError, models::database::table::TableInfoResponse,
    services::database::table::info as table_info_service,
};

/// テーブル情報の取得
///
/// **必要な権限**: `table` / `read`
///
/// 指定したテーブルの詳細情報を取得します。
#[utoipa::path(
    get,
    path = "/databases/{db_name}/tables/{table_name}",
    params(
        ("db_name" = String, Path, description = "データベース名", example = "example_database"),
        ("table_name" = String, Path, description = "テーブル名", example = "example_table")
    ),
    responses(
        (status = 200, body = TableInfoResponse),
        (status = 404, description = "テーブルまたはデータベースが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Tables"
)]
#[tracing::instrument(skip_all)]
pub async fn table_info(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
) -> Result<Json<TableInfoResponse>, AppError> {
    crate::middleware::auth::check_table(
        &app_state,
        &auth_user,
        &db_name,
        &table_name,
        crate::models::users::UserRole::Read,
    )?;

    let res = table_info_service::info(&app_state, &db_name, &table_name).await?;
    Ok(Json(res))
}
