use crate::services::database::table::data::remove as data_remove_service;
use crate::{AppState, error::AppError, models::database::table::data::RemoveDataRequest};
use axum::http::StatusCode;
use axum::{
    Json,
    extract::{Path, State},
};

#[utoipa::path(
    delete,
    path = "/databases/{db_name}/tables/{table_name}/data",
    request_body = RemoveDataRequest,
    responses(
        (status = 204, description = "Data removed"),
        (status = 404, description = "Table not found")
    ),
    tag = "tables"
)]
pub async fn data_remove(
    State(app_state): State<AppState>,
    Path((db_name, table_name)): Path<(String, String)>,
    Json(payload): Json<RemoveDataRequest>,
) -> Result<StatusCode, AppError> {
    data_remove_service::remove(
        &app_state,
        &db_name,
        &table_name,
        payload.query,
        &payload.zoom_level_policy,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
