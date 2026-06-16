mod data;

use crate::AppState;
use axum::Router;
use axum::routing::{get, post};

fn table_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/{table_name}",
            get(crate::handlers::database::table::info::table_info)
                .delete(crate::handlers::database::table::remove::remove_table),
        )
        .route(
            "/",
            post(crate::handlers::database::table::create::table_create)
                .get(crate::handlers::database::table::list::table_list),
        )
}

pub fn routes() -> Router<AppState> {
    Router::new().merge(table_routes()).merge(data::routes())
}
