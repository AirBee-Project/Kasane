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
        (status = 200, body = GetDataResponse),
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
    Query(query_params): Query<GetDataQuery>,
    Json(payload): Json<ExecuteQueryRequest>,
) -> Result<Json<GetDataResponse>, AppError> {
    // クエリ式が参照する全テーブルに Read 権限が必要。
    // データベース単位ではなくテーブル単位で検査するので、テーブルスコープの権限しか
    // 持たないユーザーでも、そのテーブルだけを参照するクエリなら実行できる。
    //
    // 検査そのものはサービス層が参照テーブルを解決するのと同じ読み取りの中で行う
    // （`services::query::resolve_tables`）。ここで別途 `check_tables` を呼ぶと、
    // 同じカタログを 2 度引くことになる。
    let result = query_service::execute(&app_state, &auth_user, payload, &query_params).await?;
    Ok(Json(result))
}
