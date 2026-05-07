use crate::{AppState, models::table::value::InsertValueRequest};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::{error::AppError, services::table::value::insert as value_insert_service};

#[utoipa::path(
    post,
    path = "/tables/{name}/values",
    params(
        ("name" = String, Path, description = "Table name")
    ),
    responses(
        (status = 201, description = "Value Inserted"),
        (status = 404, description = "Table not found")
    ),
    tag = "tables"
)]
pub async fn value_insert(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<InsertValueRequest>,
) -> Result<StatusCode, AppError> {
    value_insert_service::value_insert(&app_state, &name, payload.query, payload.value).await?;
    Ok(StatusCode::CREATED)
}
