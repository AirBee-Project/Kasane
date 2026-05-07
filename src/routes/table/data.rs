use crate::AppState;
use axum::Router;
use axum::routing::{post, put};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/{name}/data/search",
            post(crate::handlers::table::value::get::value_get),
        )
        .route(
            "/{name}/data",
            put(crate::handlers::table::value::insert::value_insert)
                .patch(crate::handlers::table::value::upsert::value_upsert)
                .delete(crate::handlers::table::value::remove::value_remove),
        )
}
