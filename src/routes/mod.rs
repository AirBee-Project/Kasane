// src/routes/mod.rs
use axum::Router;

use crate::AppState;
mod layer;
mod openapi;

use tower_http::trace::TraceLayer;

pub fn create_router(app_state: AppState) -> Router {
    Router::new()
        .nest("/layers", layer::routes())
        .merge(openapi::routes())
        .layer(TraceLayer::new_for_http())
        .with_state(app_state)
}
