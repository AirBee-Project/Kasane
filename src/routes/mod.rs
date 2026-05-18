// src/routes/mod.rs
use axum::{Router, middleware};

use crate::{AppState, auth::auth_middleware};
mod layer;
mod openapi;

use tower_http::trace::TraceLayer;

pub fn create_router(app_state: AppState) -> Router {
    Router::new()
        .nest("/layers", layer::routes())
        .merge(openapi::routes())
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state)
}
