use crate::{AppState, auth::RequireWrite, models::layer::data::InsertDataRequest};
use crate::{error::AppError, services::layer::data::insert as data_insert_service};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

#[utoipa::path(
    put,
    path = "/layers/{name}/data",
    params(("name" = String, Path, description = "Layer name")),
    request_body = InsertDataRequest,
    responses(
        (status = 201, description = "Data Inserted"),
        (status = 404, description = "Layer not found")
    ),
    tag = "layers"
)]
pub async fn data_insert(
    _auth: RequireWrite,
    State(app_state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<InsertDataRequest>,
) -> Result<StatusCode, AppError> {
    data_insert_service::insert(
        &app_state,
        &name,
        payload.query,
        payload.value,
        &payload.zoom_level_policy,
    )
    .await?;
    Ok(StatusCode::OK)
}
