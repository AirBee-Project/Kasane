use crate::services::layer::data::remove as data_remove_service;
use crate::{AppState, error::AppError, models::layer::data::RemoveDataRequest};
use axum::http::StatusCode;
use axum::{
    Json,
    extract::{Path, State},
};

#[utoipa::path(
    delete,
    path = "/layers/{name}/data",
    params(("name" = String, Path, description = "Layer name")),
    responses(
        (status = 204, description = "Data Removed"),
        (status = 404, description = "Layer not found")
    ),
    tag = "layers"
)]
pub async fn data_remove(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<RemoveDataRequest>,
) -> Result<StatusCode, AppError> {
    data_remove_service::remove(&app_state, &name, payload.query, &payload.zoom_level_policy)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
