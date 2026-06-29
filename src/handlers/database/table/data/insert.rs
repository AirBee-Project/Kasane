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

/// 新規データの挿入 (Insert)
///
/// 空間IDの配列とデータを指定し、テーブルに**新規データとして追加**します。
///
/// **注意**: 指定した空間IDがすでにテーブル内に存在する場合、データの上書きは行われずエラーとなります。
/// 「誤って既存のデータを上書きしたくない」「確実に新しいデータのみを登録したい」という厳密なユースケースに利用してください。
///
/// （※既存のデータを上書きしても良い場合は Upsert `/data` (PATCH) を使用してください）
///
/// この操作はデータベースのWrite以上の権限が必要です。
#[utoipa::path(
    put,
    path = "/databases/{db_name}/tables/{table_name}/data",
    request_body = InsertDataRequest,
    responses(
        (status = 200, description = "データ挿入成功"),
        (status = 404, description = "テーブルが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "data"
)]
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
        &payload.zoom_level_policy,
    )
    .await?;
    Ok(StatusCode::OK)
}
