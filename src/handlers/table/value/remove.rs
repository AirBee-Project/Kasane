use crate::{AppState, error::AppError, models::table::value::RemoveValueRequest};

use crate::services::table::value::remove as value_remove_service;

use axum::http::StatusCode;
use axum::{
    Json,
    extract::{Path, State},
};

#[utoipa::path(
    delete,
    path = "/layers/{name}/data",
    params(
        ("name" = String, Path, description = "Layer name")
    ),
    responses(
        (status = 204, description = "Value Removed"),
        (status = 404, description = "Layer not found")
    ),
    tag = "layers"
)]
pub async fn value_remove(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<RemoveValueRequest>,
) -> Result<StatusCode, AppError> {
    value_remove_service::value_remove(&app_state, &name, payload.query).await?;
    Ok(StatusCode::NO_CONTENT)
}
