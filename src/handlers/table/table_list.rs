use axum::{extract::State, http::StatusCode};

use crate::{AppState, error::AppError};

pub async fn list(State(app_state): State<AppState>) -> Result<StatusCode, AppError> {
    todo!()
}
