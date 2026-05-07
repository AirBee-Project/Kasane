use crate::AppState;
use axum::Router;
use axum::routing::{post, put};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/{name}/data/search",
            post(crate::handlers::layer::data::get::data_get),
        )
        .route(
            "/{name}/data",
            put(crate::handlers::layer::data::insert::data_insert)
                .patch(crate::handlers::layer::data::upsert::data_upsert)
                .delete(crate::handlers::layer::data::remove::data_remove),
        )
}
