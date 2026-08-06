// src/routes/mod.rs
use axum::{
    Router, middleware,
    routing::{delete, get, post, put},
};

use crate::AppState;
mod database;
mod openapi;

pub fn create_router(app_state: AppState) -> Router {
    let protected_router = Router::new()
        .route(
            "/system/info",
            get(crate::handlers::system::get_system_info),
        )
        .route("/users", get(crate::handlers::users::list_users))
        .route("/users", post(crate::handlers::users::create_user))
        .route(
            "/users/{username}",
            delete(crate::handlers::users::delete_user).get(crate::handlers::users::get_user),
        )
        .route(
            "/users/{username}/password",
            put(crate::handlers::users::update_password),
        )
        .route(
            "/users/{username}/admin",
            put(crate::handlers::users::set_admin),
        )
        .route(
            "/users/{username}/privileges",
            get(crate::handlers::users::get_privileges),
        )
        .route(
            "/users/{username}/privileges/{db_name}",
            put(crate::handlers::users::set_privilege),
        )
        .route(
            "/users/{username}/privileges/{db_name}",
            delete(crate::handlers::users::delete_privilege),
        )
        .route("/query", post(crate::handlers::query::execute_query))
        .nest("/databases", database::routes())
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            crate::middleware::auth::require_auth,
        ));

    let auth_router = Router::new().route("/auth/login", post(crate::handlers::auth::login));

    #[allow(unused_mut)]
    let mut router = Router::new()
        .merge(auth_router)
        .merge(protected_router)
        .merge(openapi::routes());

    // OpenTelemetryが有効な場合のみミドルウェアを追加する
    #[cfg(feature = "production")]
    if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
        router =
            router.route_layer(axum_tracing_opentelemetry::middleware::OtelAxumLayer::default());
    }

    router.with_state(app_state)
}
