// src/routes/mod.rs
use axum::Router;

use crate::AppState;
mod openapi;
mod table;

pub fn create_router(app_state: AppState) -> Router {
    Router::new()
        .nest("/tables", table::routes())
        .merge(openapi::routes())
        .with_state(app_state)
}
