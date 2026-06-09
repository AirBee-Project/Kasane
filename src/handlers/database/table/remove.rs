use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::{AppState, error::AppError, services::database::table::remove as table_remove_service};

#[utoipa::path(
    delete,
    path = "/databases/{db_name}/tables/{table_name}",
    responses(
        (status = 204, description = "Table removed"),
        (status = 404, description = "Table not found")
    ),
    tag = "tables"
)]
pub async fn table_remove(
    State(app_state): State<AppState>,
    Path((db_name, table_name)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    table_remove_service::remove(&app_state, &db_name, &table_name).await?;
    Ok(StatusCode::NO_CONTENT)
}
