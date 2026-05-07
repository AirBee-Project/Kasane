use crate::AppState;
use axum::Router;
use axum::routing::post;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/{name}/values/search",
            post(crate::handlers::table::value::get::value_get),
        )
        .route(
            "/{name}/values",
            post(crate::handlers::table::value::insert::value_insert)
                .delete(crate::handlers::table::value::remove::value_remove),
        )
}
