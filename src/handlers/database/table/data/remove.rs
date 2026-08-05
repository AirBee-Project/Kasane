use crate::middleware::auth::AuthUser;
use crate::models::users::UserRole;
use crate::services::database::table::data::remove as data_remove_service;
use crate::{AppState, error::AppError, models::database::table::data::RemoveDataRequest};
use axum::Extension;
use axum::http::StatusCode;
use axum::{
    Json,
    extract::{Path, State},
};

/// データの削除
///
/// 指定した空間IDをテーブルからを削除します。この操作はデータベースのWrite以上の権限が必要です。
#[utoipa::path(
    delete,
    path = "/databases/{db_name}/tables/{table_name}/data",
    params(
        ("db_name" = String, Path, description = "データベース名", example = "example_database"),
        ("table_name" = String, Path, description = "テーブル名", example = "example_table")
    ),
    request_body = RemoveDataRequest,
    responses(
        (status = 204),
        (status = 404, description = "テーブルが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Data"
)]
#[tracing::instrument(skip_all)]
pub async fn data_remove(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
    Json(payload): Json<RemoveDataRequest>,
) -> Result<StatusCode, AppError> {
    crate::middleware::auth::check_privilege(&app_state, &auth_user, &db_name, UserRole::Write)
        .await?;

    data_remove_service::remove(&app_state, &db_name, &table_name, &payload.spatial_ids).await?;
    Ok(StatusCode::NO_CONTENT)
}
