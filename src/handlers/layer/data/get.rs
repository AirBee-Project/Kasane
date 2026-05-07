use crate::AppState;
use axum::{Json, extract::{Path, State}};
use crate::{error::AppError, services::layer::data::get as data_get_service};

#[utoipa::path(
    post,
    path = "/layers/{name}/data/search",
    params(("name" = String, Path, description = "Layer name")),
    responses((status = 404, description = "Layer not found")),
    tag = "layers"
)]
pub async fn data_get(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<crate::models::layer::data::GetDataRequest>,
) -> Result<Json<crate::models::layer::data::GetDataResponse>, AppError> {
    let response = data_get_service::get(&app_state, &name, payload.query).await?;
    Ok(Json(response))
}
