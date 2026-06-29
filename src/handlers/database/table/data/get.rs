use crate::AppState;
use crate::middleware::auth::AuthUser;
use crate::models::database::table::data::{
    GetDataQuery, GetDataRequest, GetDataResponse, OutputFormat,
};
use crate::{error::AppError, services::database::table::data::get as data_get_service};
use axum::Extension;
use axum::{
    Json,
    extract::{Path, Query, State},
};

/// データの一括取得
///
/// 空間IDの配列を指定し、それらに紐づくデータをJSON配列として一括で取得します。この操作はデータベースのRead以上の権限が必要です。
#[utoipa::path(
    post,
    path = "/databases/{db_name}/tables/{table_name}/data/search",
    request_body = GetDataRequest,
    params(
        ("format" = Option<OutputFormat>, Query, description = "出力フォーマット (singleId, rangeId, flexId)"),
        ("limit" = Option<usize>, Query, description = "最大取得件数")
    ),
    responses(
        (status = 200, description = "データ取得成功", body = GetDataResponse),
        (status = 404, description = "テーブルが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "data"
)]
pub async fn data_get(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
    Query(query): Query<GetDataQuery>,
    Json(payload): Json<GetDataRequest>,
) -> Result<Json<crate::models::database::table::data::GetDataResponse>, AppError> {
    crate::middleware::auth::check_privilege(
        &app_state,
        &auth_user,
        &db_name,
        crate::models::users::UserRole::Read,
    )
    .await?;

    let result = data_get_service::get(
        &app_state,
        &db_name,
        &table_name,
        &payload.spatial_ids,
        &payload.zoom_level_policy,
        &query,
    )
    .await?;
    Ok(Json(result))
}
