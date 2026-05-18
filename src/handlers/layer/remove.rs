use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::{AppState, auth::RequireWrite, error::AppError, services::layer::remove as layer_remove_service};

#[utoipa::path(
    delete,
    path = "/layers/{name}",
    params(
        ("name" = String, Path, description = "Layer name")
    ),
    responses(
        (status = 204, description = "Layer deleted"),
        (status = 404, description = "Layer not found")
    ),
    tag = "layers"
)]
pub async fn layer_remove(
    _auth: RequireWrite,
    State(app_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    layer_remove_service::remove(&app_state, &name).await?;
    Ok(StatusCode::NO_CONTENT)
}
