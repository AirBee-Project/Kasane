use std::sync::Arc;
use tokio::sync::Semaphore;

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
    /// クエリの同時実行数を絞る
    pub query_concurrency: Arc<Semaphore>,
}

impl AppState {
    pub fn new(db: backend::Db) -> Self {
        let writes = services::database::table::data::coalesce::WriteCoalescer::new(db.clone());
        let query_concurrency = Arc::new(Semaphore::new(query_concurrency_limit()));
        Self {
            db,
            writes,
            query_concurrency,
        }
    }
}

/// `KASANE_QUERY_CONCURRENCY` で上書き可能。既定は論理コア数。
fn query_concurrency_limit() -> usize {
    std::env::var("KASANE_QUERY_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(4)
}

pub fn kasane(app_state: AppState) -> axum::Router {
    create_router(app_state)
}
