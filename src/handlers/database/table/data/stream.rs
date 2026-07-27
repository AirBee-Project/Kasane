use crate::{
    AppState,
    error::AppError,
    middleware::auth::AuthUser,
    models::database::table::data::{GetDataQuery, GetDataRequest, OutputFormat},
    services::database::table::data::stream::get_stream,
};
use axum::body::Bytes;
use axum::{
    Extension, Json,
    body::Body,
    extract::{Path, Query, State},
    http::{Response, StatusCode, header},
};
use tokio_stream::StreamExt;

/// データのストリーミング取得
///
/// 空間IDと値をNDJSON形式のストリームとして順次取得します。この操作はデータベースのRead以上の権限が必要です。
#[utoipa::path(
    post,
    path = "/databases/{db_name}/tables/{table_name}/data/search/stream",
    request_body = GetDataRequest,
    params(
        ("db_name" = String, Path, description = "データベース名", example = "example_database"),
        ("table_name" = String, Path, description = "テーブル名", example = "example_table"),
        ("format" = Option<OutputFormat>, Query, description = "出力フォーマット(singleId, rangeId, flexId)"),
        ("limit" = Option<usize>, Query, description = "最大取得件数")
    ),
    responses(
        (status = 200, body = String)
    ),
    security(("bearer_auth" = [])),
    tag = "Data"
)]
#[tracing::instrument(skip_all)]
pub async fn data_get_stream(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
    Query(query): Query<GetDataQuery>,
    Json(payload): Json<GetDataRequest>,
) -> Result<Response<Body>, AppError> {
    crate::middleware::auth::check_privilege(
        &app_state,
        &auth_user,
        &db_name,
        crate::models::users::UserRole::Read,
    )
    .await?;

    let stream = get_stream(
        &app_state,
        &db_name,
        &table_name,
        &payload.spatial_ids,
        &payload.zoom_level_policy,
        &query,
    )
    .await?;

    let stream_bytes = stream.map(|res| match res {
        Ok(str) => Ok::<_, axum::Error>(Bytes::from(str)),
        Err(e) => {
            let err_str = serde_json::to_string(&serde_json::json!({"error": e.to_string()}))
                .unwrap_or_default()
                + "\n";
            Ok(Bytes::from(err_str))
        }
    });

    let body = Body::from_stream(stream_bytes);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-ndjson")
        .body(body)
        .unwrap())
}
