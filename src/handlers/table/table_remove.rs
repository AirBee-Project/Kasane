use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::{AppState, error::AppError, services::table::table_remove as table_remove_service};

#[utoipa::path(
    delete,
    path = "/layers/{name}",
    params(
        ("name" = String, Path, description = "Table name")
    ),
    responses(
        (status = 204, description = "Table deleted"),
        (status = 404, description = "Table not found")
    ),
    tag = "layers"
)]
pub async fn table_remove(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    table_remove_service::remove(&app_state, &name).await?;
    Ok(StatusCode::NO_CONTENT)
}
