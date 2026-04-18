use crate::routes::create_router;

pub mod error;
pub mod handlers;
pub mod models;
pub mod repositories;
pub mod routes;

#[derive(Debug, Clone)]
pub struct AppState {}

pub fn kasane(app_state: AppState) -> axum::Router {
    create_router(app_state)
}
