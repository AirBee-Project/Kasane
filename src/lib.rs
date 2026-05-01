use std::sync::Arc;

use redb::Database;

use crate::routes::create_router;

pub mod db_init;
pub mod error;
pub mod handlers;
pub mod models;
pub mod repositories;
pub mod routes;

#[derive(Debug, Clone)]
pub struct AppState {
    pub redb: Arc<Database>,
}

pub fn kasane(app_state: AppState) -> axum::Router {
    create_router(app_state)
}
