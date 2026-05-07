use crate::AppState;
use axum::Router;
use axum::routing::{get, post};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/{name}/values/search",
            post(crate::handlers::table::value_get::value_get),
        )
        .route(
            "/{name}/values",
            post(crate::handlers::table::value_insert::value_insert)
                .delete(crate::handlers::table::value_remove::value_remove),
        )
        .route(
            "/{name}",
            get(crate::handlers::table::table_info::table_info)
                .delete(crate::handlers::table::table_remove::table_remove),
        )
        .route(
            "/",
            post(crate::handlers::table::table_create::table_create)
                .get(crate::handlers::table::table_list::table_list),
        )
}
