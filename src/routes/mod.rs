// src/routes/mod.rs
use axum::Router;

use crate::AppState;
mod table;

pub fn create_router(app_state: AppState) -> Router {
    Router::new()
        .nest("/table", table::routes())
        .with_state(app_state)
}
