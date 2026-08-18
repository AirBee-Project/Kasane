use crate::middleware::auth::AuthUser;
use crate::{AppState, models::database::table::data::InsertDataRequest};
use crate::{error::AppError, services::database::table::data::upsert as data_upsert_service};
use axum::Extension;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

/// データの部分追加
///
/// **必要な権限**: `table` / `write`
///
/// 指定した空間IDに対して、指定した値を書き込みます。
///
/// - **指定した空間IDがすでに存在する場合**: 既存のデータは維持されます。
/// - **指定した空間IDが存在しない場合**: データが書き込まれます。
#[utoipa::path(
    patch,
    path = "/databases/{db_name}/tables/{table_name}/data",
    params(
        ("db_name" = String, Path, description = "データベース名", example = "example_database"),
        ("table_name" = String, Path, description = "テーブル名", example = "example_table")
    ),
    request_body = InsertDataRequest,
    responses(
        (status = 204),
        (status = 400, description = "Tableに対して時空間IDが不正な場合（ズームレベルの不一致や時間の有無の不一致）"),
        (status = 404, description = "テーブルが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Data"
)]
#[tracing::instrument(skip_all)]
pub async fn data_upsert(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
    Json(payload): Json<InsertDataRequest>,
) -> Result<StatusCode, AppError> {
    data_upsert_service::upsert(
        &app_state,
        &auth_user,
        &db_name,
        &table_name,
        &payload.spatial_ids,
        payload.value,
        &payload.zoom_level_policy,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
