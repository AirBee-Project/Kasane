use crate::AppState;
use axum::Router;
use axum::routing::{get, post};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/{name}",
            get(crate::handlers::layer::info::layer_info)
                .delete(crate::handlers::layer::remove::layer_remove),
        )
        .route(
            "/",
            post(crate::handlers::layer::create::layer_create)
                .get(crate::handlers::layer::list::layer_list),
        )
}
