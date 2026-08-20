// src/routes/mod.rs
use axum::{
    Router, middleware,
    routing::{delete, get, post, put},
};
use utoipa::OpenApi;

use crate::AppState;

pub fn create_router(app_state: AppState) -> Router {
    let protected_router = Router::new()
        // System
        .route(
            "/system/info",
            get(crate::handlers::system::get_system_info),
        )
        // Databases
        .route(
            "/databases",
            post(crate::handlers::database::database_create)
                .get(crate::handlers::database::database_list),
        )
        .route(
            "/databases/{db_name}",
            get(crate::handlers::database::database_info)
                .delete(crate::handlers::database::remove_database)
                .patch(crate::handlers::database::database_update),
        )
        .route(
            "/databases/{db_name}/copy",
            post(crate::handlers::database::database_copy),
        )
        // Tables
        .route(
            "/databases/{db_name}/tables",
            post(crate::handlers::database::table::create::table_create)
                .get(crate::handlers::database::table::list::table_list),
        )
        .route(
            "/databases/{db_name}/tables/{table_name}",
            get(crate::handlers::database::table::info::table_info)
                .patch(crate::handlers::database::table::update::table_update_handler)
                .delete(crate::handlers::database::table::remove::remove_table),
        )
        .route(
            "/databases/{db_name}/tables/{table_name}/copy",
            post(crate::handlers::database::table::copy::table_copy),
        )
        // Data
        .route(
            "/databases/{db_name}/tables/{table_name}/data/search",
            post(crate::handlers::database::table::data::get::data_get),
        )
        .route(
            "/databases/{db_name}/tables/{table_name}/data",
            put(crate::handlers::database::table::data::insert::data_insert)
                .patch(crate::handlers::database::table::data::upsert::data_upsert)
                .delete(crate::handlers::database::table::data::remove::data_remove),
        )
        // Query
        .route("/query", post(crate::handlers::query::execute_query))
        // Users
        .route(
            "/users",
            get(crate::handlers::users::list_users).post(crate::handlers::users::create_user),
        )
        .route(
            "/users/{username}",
            delete(crate::handlers::users::delete_user).get(crate::handlers::users::get_user),
        )
        .route(
            "/users/{username}/password",
            put(crate::handlers::users::update_password),
        )
        .route(
            "/users/{username}/privileges",
            get(crate::handlers::users::get_privileges),
        )
        .route(
            "/users/{username}/privileges/global",
            put(crate::handlers::users::set_global_privilege)
                .delete(crate::handlers::users::delete_global_privilege),
        )
        .route(
            "/users/{username}/privileges/databases/{db_name}",
            put(crate::handlers::users::set_database_privilege)
                .delete(crate::handlers::users::delete_database_privilege),
        )
        .route(
            "/users/{username}/privileges/databases/{db_name}/tables/{table_name}",
            put(crate::handlers::users::set_table_privilege)
                .delete(crate::handlers::users::delete_table_privilege),
        )
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            crate::middleware::auth::require_auth,
        ));

    // Auth & System (Unauthenticated)
    let public_router = Router::new()
        .route("/health", get(crate::handlers::system::health_check))
        .route("/auth/login", post(crate::handlers::auth::login));

    #[allow(unused_mut)]
    let mut router = Router::new()
        .merge(public_router)
        .merge(protected_router)
        .merge(
            utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
                .url("/openapi.json", crate::openapi::ApiDoc::openapi()),
        )
        .route_layer(middleware::from_fn(crate::middleware::metrics::record));

    // 本番環境ではOpenTelemetryを追加する
    #[cfg(feature = "production")]
    if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
        router = router.route_layer(middleware::from_fn(crate::middleware::scheme::normalize));
    }

    router.with_state(app_state)
}
