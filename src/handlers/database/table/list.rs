use crate::middleware::auth::AuthUser;
use axum::Extension;
use axum::{
    Json,
    extract::{Path, State},
};

use crate::{
    AppState, error::AppError, models::database::table::TableListResponse,
    services::database::table::list as table_list_service,
};

/// テーブル一覧の取得
///
/// **必要な権限**: `database` / `read`（配下テーブルの権限でも可・結果は権限で絞られる）
///
/// 指定したデータベース内に存在するテーブルのうち、呼び出したユーザーが Read 以上の権限を
/// 持つものだけを返します。
#[utoipa::path(
    get,
    path = "/databases/{db_name}/tables",
    params(
        ("db_name" = String, Path, description = "データベース名", example = "example_database")
    ),
    responses(
        (status = 200, body = TableListResponse)
    ),
    security(("bearer_auth" = [])),
    tag = "Tables"
)]
#[tracing::instrument(skip_all)]
pub async fn table_list(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(db_name): Path<String>,
) -> Result<Json<TableListResponse>, AppError> {
    crate::middleware::auth::check_database_visible(&app_state, &auth_user, &db_name)?;

    let tables = table_list_service::list(&app_state, &db_name, &auth_user).await?;
    Ok(Json(tables))
}
