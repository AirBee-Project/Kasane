use crate::AppState;
use axum::Router;
use axum::routing::{delete, get, post};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/{name}/insert",
            post(crate::handlers::table::insert::insert),
        )
        .route("/{name}", get(crate::handlers::table::info::info))
        .route("/{name}", delete(crate::handlers::table::remove::remove))
        .route("/", post(crate::handlers::table::create::create))
}
