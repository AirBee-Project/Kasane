use crate::AppState;
use axum::Router;
use axum::routing::get;

pub fn routes() -> Router<AppState> {
    Router::new().route("/{name}", get(crate::handlers::table::info::info))
}
