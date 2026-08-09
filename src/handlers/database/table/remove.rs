use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::middleware::auth::AuthUser;
use crate::models::users::UserRole;
use crate::{AppState, error::AppError, services::database::table::remove as table_remove_service};
use axum::Extension;

/// テーブルの削除
///
/// **必要な権限**: `table` / `manage`
///
/// 指定したテーブルを削除します。
#[utoipa::path(
    delete,
    path = "/databases/{db_name}/tables/{table_name}",
    params(
        ("db_name" = String, Path, description = "データベース名", example = "example_database"),
        ("table_name" = String, Path, description = "テーブル名", example = "example_table")
    ),
    responses(
        (status = 204),
        (status = 404, description = "テーブルが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Tables"
)]
#[tracing::instrument(skip_all)]
pub async fn remove_table(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    crate::middleware::auth::check_table(
        &app_state,
        &auth_user,
        &db_name,
        &table_name,
        UserRole::Manage,
    )?;
    table_remove_service::remove(&app_state, &db_name, &table_name).await?;
    Ok(StatusCode::NO_CONTENT)
}
