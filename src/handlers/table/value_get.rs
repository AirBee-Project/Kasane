use crate::{AppState, models::value::GetValueRequest};

use axum::{
    Json,
    extract::{Path, State},
};

use crate::{error::AppError, services::table::value_get as value_get_service};

#[utoipa::path(
    post,
    path = "/tables/{name}/values/search",
    params(
        ("name" = String, Path, description = "Table name")
    ),
    responses(
        (status = 404, description = "Table not found")
    ),
    tag = "tables"
)]
pub async fn value_get(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<GetValueRequest>,
) -> Result<Json<crate::models::value::response::GetValueResponse>, AppError> {
    let response = value_get_service::value_get(&app_state, &name, payload.query).await?;
    Ok(Json(response))
}
