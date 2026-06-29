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
/// 指定したデータベース内に存在するすべてのテーブルの一覧を取得します。この操作はデータベースのRead以上の権限が必要です。
#[utoipa::path(
    get,
    path = "/databases/{db_name}/tables",
    responses(
        (status = 200, description = "テーブル一覧取得成功", body = TableListResponse)
    ),
    security(("bearer_auth" = [])),
    tag = "tables"
)]
pub async fn table_list(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(db_name): Path<String>,
) -> Result<Json<TableListResponse>, AppError> {
    crate::middleware::auth::check_privilege(
        &app_state,
        &auth_user,
        &db_name,
        crate::models::users::UserRole::Read,
    )
    .await?;

    let tables = table_list_service::list(&app_state, &db_name).await?;
    Ok(Json(tables))
}
