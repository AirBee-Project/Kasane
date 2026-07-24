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
    users::UserRole,
};
use crate::services::query as query_service;

/// クエリの実行
///
/// 複数のテーブルを対象にクエリ式を評価し、指定した空間IDの結果を取得します。
/// クエリ式が参照する**すべてのデータベース**に対して Read 以上の権限が必要です。
#[utoipa::path(
    post,
    path = "/query",
    request_body = ExecuteQueryRequest,
    params(
        ("format" = Option<OutputFormat>, Query, description = "出力フォーマット(singleId, rangeId, flexId)"),
        ("limit" = Option<usize>, Query, description = "最大取得件数")
    ),
    responses(
        (status = 200, body = GetDataResponse),
        (status = 400, description = "クエリ式が不正（型の混在・非対応の型など）"),
        (status = 403, description = "参照先データベースへの権限が不足"),
        (status = 404, description = "参照先のテーブルが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Query"
)]
pub async fn execute_query(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Query(query_params): Query<GetDataQuery>,
    Json(payload): Json<ExecuteQueryRequest>,
) -> Result<Json<GetDataResponse>, AppError> {
    // クエリ式が参照する全データベースに Read 権限が必要。
    // 重複を除いてから検査する（同じDBを何度も引かない）。
    let mut checked: Vec<&str> = Vec::new();
    for (db_name, _table) in payload.query.sources() {
        if checked.contains(&db_name) {
            continue;
        }
        crate::middleware::auth::check_privilege(&app_state, &auth_user, db_name, UserRole::Read)
            .await?;
        checked.push(db_name);
    }

    let result = query_service::execute(&app_state, payload, &query_params).await?;
    Ok(Json(result))
}
