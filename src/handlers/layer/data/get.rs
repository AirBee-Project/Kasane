use crate::AppState;
use crate::{auth::RequireRead, error::AppError, services::layer::data::get as data_get_service};
use axum::{
    Json,
    extract::{Path, State},
};

#[utoipa::path(
    post,
    path = "/layers/{name}/data/search",
    params(("name" = String, Path, description = "Layer name")),
    responses((status = 404, description = "Layer not found")),
    tag = "layers"
)]
pub async fn data_get(
    _auth: RequireRead,
    State(app_state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<crate::models::layer::data::GetDataRequest>,
) -> Result<Json<crate::models::layer::data::GetDataResponse>, AppError> {
    let response =
        data_get_service::get(&app_state, &name, payload.query, &payload.zoom_level_policy).await?;
    Ok(Json(response))
}
