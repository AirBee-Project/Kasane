use crate::{
    AppState,
    error::AppError,
    middleware::auth::AuthUser,
    models::database::table::{TableSummary, UpdateTableRequest},
    services::database::table::update as table_update_service,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};

/// テーブルの更新
///
/// **必要な権限**: `table` / `manage`
///
/// テーブルの名前変更や値の制約を更新します。
/// 権限はテーブル ID に紐づくため、改名しても権限は追従します。

#[utoipa::path(
    patch,
    path = "/databases/{db_name}/tables/{table_name}",
    tags = ["Tables"],
    params(
        ("db_name" = String, Path, description = "データベース名"),
        ("table_name" = String, Path, description = "テーブル名"),
    ),
    request_body = UpdateTableRequest,
    responses(
        (status = 200, description = "テーブルの更新に成功", body = TableSummary),
        (status = 400, description = "リクエストが不正（パラメータエラー、既存データが新しい制約に違反した場合、または is_temporal を false に変更しようとした場合）"),
        (status = 401, description = "認証エラー"),
        (status = 403, description = "権限が不足している"),
        (status = 404, description = "データベースまたはテーブルが見つからない"),
        (status = 409, description = "同名のテーブルが既に存在する")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[tracing::instrument(skip_all)]
pub async fn table_update_handler(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
    Json(payload): Json<UpdateTableRequest>,
) -> Result<Json<TableSummary>, AppError> {
    let result = table_update_service::table_update(
        state.clone(),
        &auth_user,
        &db_name,
        &table_name,
        payload.name.as_deref(),
        payload.constraints,
        payload.description,
        payload.is_temporal,
    )
    .await?;

    Ok(Json(result))
}
