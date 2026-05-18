use std::sync::Arc;

use redb::Database;

use crate::routes::create_router;

pub mod auth;
pub mod db_init;
pub mod error;
pub mod handlers;
pub mod models;
pub mod openapi;
pub mod repositories;
pub mod routes;
pub mod services;

#[derive(Debug, Clone)]
pub struct AppState {
    pub redb: Arc<Database>,
    pub readonly_key: Option<String>,
    pub write_key: Option<String>,
}

pub fn kasane(app_state: AppState) -> axum::Router {
    create_router(app_state)
}
