use crate::{AppState, models::layer::data::InsertDataRequest};
use crate::{error::AppError, services::layer::data::upsert as data_upsert_service};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

#[utoipa::path(
    patch,
    path = "/layers/{name}/data",
    params(("name" = String, Path, description = "Layer name")),
    request_body = InsertDataRequest,
    responses(
        (status = 204, description = "Data Upserted (written only to empty spatial IDs)"),
        (status = 404, description = "Layer not found")
    ),
    tag = "layers"
)]
pub async fn data_upsert(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<InsertDataRequest>,
) -> Result<StatusCode, AppError> {
    data_upsert_service::upsert(
        &app_state,
        &name,
        payload.query,
        payload.value,
        &payload.zoom_level_policy,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
