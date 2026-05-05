use crate::{AppState, models::table::InsertValueRequest};

use axum::{
    Json,
    extract::{Path, State},
};

pub async fn insert(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<InsertValueRequest>,
) {
}
