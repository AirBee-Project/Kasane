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
/// **必要な権限**: `table` / `read`
///
/// 空間IDと値をJSON配列として一括で取得します。空間のみのTableであっても時間を指定して読み取りを行うことができます。
#[utoipa::path(
    post,
    path = "/databases/{db_name}/tables/{table_name}/data/search",
    request_body = GetDataRequest,
    params(
        ("db_name" = String, Path, description = "データベース名", example = "example_database"),
        ("table_name" = String, Path, description = "テーブル名", example = "example_table"),
        ("format" = Option<OutputFormat>, Query, description = "出力フォーマット(singleId, rangeId, flexId)"),
        ("limit" = Option<usize>, Query, description = "最大取得件数")
    ),
    responses(
        (status = 200, body = GetDataResponse),
        (status = 400, description = "空間IDが不正な場合"),
        (status = 404, description = "テーブルが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Data"
)]
#[tracing::instrument(skip_all)]
pub async fn data_get(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
    Query(query): Query<GetDataQuery>,
    Json(payload): Json<GetDataRequest>,
) -> Result<Json<crate::models::database::table::data::GetDataResponse>, AppError> {
    // 認可はサービス層の解決と同じ読み取りで行う。別途呼ぶとカタログを 2 度引く。
    let result = data_get_service::get(
        &app_state,
        &auth_user,
        &db_name,
        &table_name,
        &payload.spatial_ids,
        &payload.zoom_level_policy,
        &query,
    )
    .await?;
    Ok(Json(result))
}
