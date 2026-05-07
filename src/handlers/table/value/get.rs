use crate::AppState;

use axum::{
    Json,
    extract::{Path, State},
};

use crate::{error::AppError, services::table::value::get as value_get_service};

#[utoipa::path(
    post,
    path = "/layers/{name}/data/search",
    params(
        ("name" = String, Path, description = "Layer name")
    ),
    responses(
        (status = 404, description = "Layer not found")
    ),
    tag = "layers"
)]
pub async fn value_get(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<crate::models::table::value::GetValueRequest>,
) -> Result<Json<crate::models::table::value::GetValueResponse>, AppError> {
    let response = value_get_service::value_get(&app_state, &name, payload.query).await?;
    Ok(Json(response))
}
