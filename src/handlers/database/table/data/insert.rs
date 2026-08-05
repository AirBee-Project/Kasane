use crate::middleware::auth::AuthUser;
use crate::models::users::UserRole;
use crate::{AppState, models::database::table::data::InsertDataRequest};
use crate::{error::AppError, services::database::table::data::insert as data_insert_service};
use axum::Extension;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

/// データの上書き・追加
///
/// 指定した空間IDに対して、指定した値を書き込みます。
/// 既に値が存在する空間IDに対しては、その値を完全に上書きします。
///
/// この操作はデータベースのWrite以上の権限が必要です。
#[utoipa::path(
    put,
    path = "/databases/{db_name}/tables/{table_name}/data",
    params(
        ("db_name" = String, Path, description = "データベース名", example = "example_database"),
        ("table_name" = String, Path, description = "テーブル名", example = "example_table")
    ),
    request_body = InsertDataRequest,
    responses(
        (status = 200),
        (status = 404, description = "テーブルが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Data"
)]
#[tracing::instrument(skip_all)]
pub async fn data_insert(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
    Json(payload): Json<InsertDataRequest>,
) -> Result<StatusCode, AppError> {
    crate::middleware::auth::check_privilege(&app_state, &auth_user, &db_name, UserRole::Write)
        .await?;

    data_insert_service::insert(
        &app_state,
        &db_name,
        &table_name,
        &payload.spatial_ids,
        payload.value,
    )
    .await?;
    Ok(StatusCode::OK)
}
