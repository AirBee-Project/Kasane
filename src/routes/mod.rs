// src/routes/mod.rs
use axum::Router;

use crate::AppState;
mod layer;
mod openapi;

pub fn create_router(app_state: AppState) -> Router {
    Router::new()
        .nest("/layers", layer::routes())
        .merge(openapi::routes())
        .with_state(app_state)
}
