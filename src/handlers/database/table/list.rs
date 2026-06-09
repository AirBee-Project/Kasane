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

#[utoipa::path(
    get,
    path = "/databases/{db_name}/tables",
    responses(
        (status = 200, description = "List of tables", body = TableListResponse)
    ),
    tag = "tables"
)]
pub async fn table_list(
    Path(db_name): Path<String>,
    State(app_state): State<AppState>,
    Extension(_auth_user): Extension<AuthUser>,
) -> Result<Json<TableListResponse>, AppError> {
    let tables = table_list_service::list(&app_state, &db_name).await?;
    Ok(Json(tables))
}
