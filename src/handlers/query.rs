use axum::{
    Extension, Json,
    extract::{Query, State},
};

use crate::AppState;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::models::{
    database::table::data::{GetDataQuery, GetDataResponse, OutputFormat},
    query::ExecuteQueryRequest,
};
use crate::services::query as query_service;

/// クエリの実行
///
/// **必要な権限**: 参照する全テーブルに `table` / `read`
///
/// 複数のテーブルを対象にクエリ式を実行し、指定した空間IDの結果を取得します。
#[utoipa::path(
    post,
    path = "/query",
    request_body = ExecuteQueryRequest,
    params(
        ("format" = Option<OutputFormat>, Query, description = "出力フォーマット(singleId, rangeId, flexId)"),
        ("limit" = Option<usize>, Query, description = "最大取得件数")
    ),
    responses(
        (status = 200, description = "クエリ実行成功", content(
            (GetDataResponse = "application/json"),
            (String = "application/vnd.apache.arrow.stream")
        )),
        (status = 400, description = "クエリ式が不正（型の混在・非対応の型など）"),
        (status = 403, description = "参照先テーブルへの権限が不足"),
        (status = 404, description = "参照先のテーブルが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Query"
)]
#[tracing::instrument(skip_all)]
pub async fn execute_query(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    headers: axum::http::HeaderMap,
    Query(query_params): Query<GetDataQuery>,
    Json(payload): Json<ExecuteQueryRequest>,
) -> Result<axum::response::Response, AppError> {
    let is_arrow = if let Some(accept) = headers.get(axum::http::header::ACCEPT) {
        accept
            .to_str()
            .unwrap_or("")
            .contains("application/vnd.apache.arrow.stream")
    } else {
        false
    };

    let result =
        query_service::execute(&app_state, &auth_user, payload, &query_params, is_arrow).await?;
    Ok(result)
}
