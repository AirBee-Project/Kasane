use crate::middleware::auth::AuthUser;
use crate::models::users::UserRole;
use crate::{AppState, models::database::table::data::InsertDataRequest};
use crate::{error::AppError, services::database::table::data::upsert as data_upsert_service};
use axum::Extension;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

/// データの更新・追加 (Upsert)
///
/// 空間IDの配列とデータを指定し、テーブルのデータを**上書きまたは新規追加**します。
///
/// - **指定した空間IDがすでに存在する場合**: 新しいデータで**上書き（更新）**されます。
/// - **指定した空間IDが存在しない場合**: **新規データとして追加**されます。
///
/// 「既存データがあれば更新し、なければ作る」というように、IDの有無を気にせず常に最新のデータを投入したいユースケースに最適です。
///
/// この操作はデータベースのWrite以上の権限が必要です。
#[utoipa::path(
    patch,
    path = "/databases/{db_name}/tables/{table_name}/data",
    request_body = InsertDataRequest,
    responses(
        (status = 204),
        (status = 404, description = "テーブルが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "data"
)]
pub async fn data_upsert(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
    Json(payload): Json<InsertDataRequest>,
) -> Result<StatusCode, AppError> {
    crate::middleware::auth::check_privilege(&app_state, &auth_user, &db_name, UserRole::Write)
        .await?;

    data_upsert_service::upsert(
        &app_state,
        &db_name,
        &table_name,
        &payload.spatial_ids,
        payload.value,
        &payload.zoom_level_policy,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
