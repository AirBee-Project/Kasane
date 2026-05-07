use crate::{AppState, error::AppError, models::table::value::RemoveValueRequest};

use crate::services::table::value::remove as value_remove_service;

use axum::http::StatusCode;
use axum::{
    Json,
    extract::{Path, State},
};

#[utoipa::path(
    delete,
    path = "/tables/{name}/values",
    params(
        ("name" = String, Path, description = "Table name")
    ),
    responses(
        (status = 204, description = "Value Removed"),
        (status = 404, description = "Table not found")
    ),
    tag = "tables"
)]
pub async fn value_remove(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<RemoveValueRequest>,
) -> Result<StatusCode, AppError> {
    value_remove_service::value_remove(&app_state, &name, payload.query).await?;
    Ok(StatusCode::NO_CONTENT)
}
