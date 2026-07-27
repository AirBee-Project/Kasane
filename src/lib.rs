use std::sync::Arc;

use crate::routes::create_router;

pub mod auth_cache;
pub mod db_init;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod openapi;
pub mod repositories;
pub mod routes;
pub mod services;

#[derive(Clone)]
pub struct AppState {
    pub db: db_init::AppDb,
    pub auth_cache: Arc<auth_cache::AuthCache>,
}

pub fn kasane(app_state: AppState) -> axum::Router {
    create_router(app_state)
}
pub mod telemetry;
