use std::sync::Arc;

use redb::Database;

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

#[derive(Debug, Clone)]
pub struct AppState {
    pub redb: Arc<Database>,
    pub auth_cache: Arc<tokio::sync::RwLock<auth_cache::AuthCache>>,
}

pub fn kasane(app_state: AppState) -> axum::Router {
    create_router(app_state)
}
