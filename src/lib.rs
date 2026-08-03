use crate::routes::create_router;

pub mod db_init;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod openapi;
pub mod repositories;
pub mod routes;
pub mod services;
pub mod telemetry;

#[derive(Clone)]
pub struct AppState {
    pub db: db_init::AppDb,
}

pub fn kasane(app_state: AppState) -> axum::Router {
    create_router(app_state)
}
