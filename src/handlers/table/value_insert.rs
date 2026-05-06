use crate::{AppState, models::value::InsertValueRequest};

use axum::{
    Json,
    extract::{Path, State},
};

pub async fn value_insert(
    State(_app_state): State<AppState>,
    Path(_name): Path<String>,
    Json(_payload): Json<InsertValueRequest>,
) {
}
