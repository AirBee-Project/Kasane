use crate::{
    AppState,
    error::AppError,
    middleware::auth::AuthUser,
    models::{
        database::table::{TableSummary, UpdateTableRequest},
        users::UserRole,
    },
    services::database::table::update as table_update_service,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};

/// テーブルの更新
///
/// テーブルの名前変更や値の制約を更新します。この操作はデータベースの Manage 権限が必要です。
///
/// 制約を変更する際、既存のデータが新しい制約に適合しているかを検証する挙動を `validate_existing_data` フラグで制御できます。
/// - `true`（デフォルト）の場合、既存データが新しい制約に違反していればエラーとして更新を却下します。
/// - `false` の場合、既存データのチェックをスキップして高速に変更を適用します（不整合なデータが残る可能性があります）。
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
        (status = 400, description = "リクエストが不正（パラメータエラー、または validate_existing_data=true の状態で既存データが新しい制約に違反した場合）"),
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
    crate::middleware::auth::check_privilege(&state, &auth_user, &db_name, UserRole::Manage)
        .await?;

    let result = table_update_service::table_update(
        state.clone(),
        &db_name,
        &table_name,
        payload.name.as_deref(),
        Some(payload.constraints),
        Some(payload.description),
        payload.validate_existing_data,
    )
    .await?;

    Ok(Json(result))
}
