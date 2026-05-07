use crate::{AppState, models::table::value::InsertValueRequest};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::{error::AppError, services::table::value::upsert as value_upsert_service};

#[utoipa::path(
    patch,
    path = "/layers/{name}/data",
    params(
        ("name" = String, Path, description = "Layer name")
    ),
    request_body = InsertValueRequest,
    responses(
        (status = 204, description = "Value Upserted (written only to empty spatial IDs)"),
        (status = 404, description = "Layer not found")
    ),
    tag = "layers"
)]
pub async fn value_upsert(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<InsertValueRequest>,
) -> Result<StatusCode, AppError> {
    value_upsert_service::value_upsert(&app_state, &name, payload.query, payload.value).await?;
    Ok(StatusCode::NO_CONTENT)
}
