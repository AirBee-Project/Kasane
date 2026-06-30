use crate::AppState;
use axum::Router;
use axum::routing::{get, post};

pub fn routes() -> Router<AppState> {
    let db_routes = Router::new()
        .route(
            "/{db_name}",
            get(crate::handlers::database::database_info)
                .delete(crate::handlers::database::remove_database)
                .patch(crate::handlers::database::database_rename),
        )
        .route(
            "/{db_name}/copy",
            post(crate::handlers::database::database_copy),
        )
        .route(
            "/",
            post(crate::handlers::database::database_create)
                .get(crate::handlers::database::database_list),
        );

    Router::new().merge(db_routes).nest(
        "/{db_name}/tables",
        crate::routes::database::table::routes(),
    )
}

pub mod table;
