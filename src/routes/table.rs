use crate::AppState;
use axum::Router;
use axum::routing::{delete, get, post};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/{name}/insert",
            post(crate::handlers::table::value_insert::value_insert),
        )
        .route("/{name}", get(crate::handlers::table::table_info::info))
        .route(
            "/{name}",
            delete(crate::handlers::table::table_remove::remove),
        )
        .route("/", post(crate::handlers::table::table_create::create).get(crate::handlers::table::table_list::list))
}
