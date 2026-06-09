use crate::AppState;
use axum::Router;
use axum::routing::{post, put};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/{name}/data/search",
            post(crate::handlers::database::table::data::get::data_get),
        )
        .route(
            "/{name}/data",
            put(crate::handlers::database::table::data::insert::data_insert)
                .patch(crate::handlers::database::table::data::upsert::data_upsert)
                .delete(crate::handlers::database::table::data::remove::data_remove),
        )
}
