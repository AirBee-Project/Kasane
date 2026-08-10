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
///
/// 保持するのはビルド時に選択されたストレージ 1 つだけで、その中身は
/// [`Storage`](crate::repositories::Storage) 越しにしか触れない。
#[derive(Clone)]
pub struct AppState {
    pub db: backend::Db,
}

pub fn kasane(app_state: AppState) -> axum::Router {
    create_router(app_state)
}
