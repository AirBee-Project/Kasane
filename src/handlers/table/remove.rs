use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    AppState,
    error::AppError,
    services::table::remove as table_remove_service,
};

pub async fn remove(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    table_remove_service::remove(&app_state, &name).await?;

    Ok(StatusCode::NO_CONTENT)
}