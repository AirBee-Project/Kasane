use crate::routes::create_router;

pub mod backend;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod openapi;
pub mod repositories;
pub mod routes;
pub mod services;
pub mod telemetry;

/// リクエスト間で共有する状態。
#[derive(Clone)]
pub struct AppState {
    pub db: backend::Db,
    pub writes: services::database::table::data::coalesce::WriteCoalescer,
}

impl AppState {
    pub fn new(db: backend::Db) -> Self {
        let writes = services::database::table::data::coalesce::WriteCoalescer::new(db.clone());
        Self { db, writes }
    }
}

pub fn kasane(app_state: AppState) -> axum::Router {
    create_router(app_state)
}
