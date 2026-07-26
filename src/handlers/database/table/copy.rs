use crate::middleware::auth::AuthUser;
use crate::models::users::UserRole;
use axum::Extension;
use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};

use crate::{
    AppState,
    error::AppError,
    models::database::table::{CopyTableRequest, TableSummary},
    services::database::table::copy as table_copy_service,
};

/// テーブルのコピー
///
/// 指定したテーブルを別のデータベース、または同じデータベース内に異なる名前でコピーします。
/// コピー元データベースに対するRead権限と、コピー先データベースに対するManage権限が必要です。
#[utoipa::path(
    post,
    path = "/databases/{db_name}/tables/{table_name}/copy",
    params(
        ("db_name" = String, Path, description = "コピー元データベース名", example = "source_database"),
        ("table_name" = String, Path, description = "コピー元テーブル名", example = "source_table")
    ),
    request_body = CopyTableRequest,
    responses(
        (status = 200, body = TableSummary, description = "成功"),
        (status = 400, description = "リクエストが不正（パラメータエラー、または不正なテーブル名）"),
        (status = 403, description = "権限不足（コピー元DBのRead権限、またはコピー先DBのManage権限がない場合）"),
        (status = 404, description = "コピー元データベース、コピー元テーブル、またはコピー先データベースが存在しない"),
        (status = 409, description = "コピー先テーブルがすでに存在する")
    ),
    security(("bearer_auth" = [])),
    tag = "Tables"
)]
pub async fn table_copy(
    Path((db_name, table_name)): Path<(String, String)>,
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(request): Json<CopyTableRequest>,
) -> Result<impl IntoResponse, AppError> {
    let dest_db_name = request.copy_db_name.as_deref().unwrap_or(&db_name);

    // 1. コピー元データベースのRead権限チェック
    crate::middleware::auth::check_privilege(&app_state, &auth_user, &db_name, UserRole::Read)
        .await?;

    // 2. コピー先データベースのManage権限チェック（新しくテーブルを作成するため）
    crate::middleware::auth::check_privilege(
        &app_state,
        &auth_user,
        dest_db_name,
        UserRole::Manage,
    )
    .await?;

    let res = table_copy_service::copy(
        &app_state,
        &db_name,
        &table_name,
        dest_db_name,
        &request.copy_table_name,
    )
    .await?;

    let summary = TableSummary {
        name: res.name,
        data_type: res.data_type,
        max_zoom_level: res.max_zoom_level,
        constraints: res.constraints,
    };

    Ok(Json(summary))
}
