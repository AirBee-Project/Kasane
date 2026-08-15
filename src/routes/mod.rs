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
        .merge(openapi::routes())
        // `route_layer` なので、実際にどれかのルートへ一致したリクエストだけが通る
        // （`MatchedPath` を使うのはそのため。ヘルスチェックの取りこぼしなどは
        // 一致しないので計測に乗らない）。ビルド／環境を問わず常に張る:
        // 本番以外では `telemetry::metrics::*` が無コストな no-op になるだけなので、
        // 「本番だけ計測される」という分岐を運用側に持たせない。
        .route_layer(middleware::from_fn(crate::middleware::metrics::record));

    // OpenTelemetryが有効な場合のみミドルウェアを追加する
    #[cfg(feature = "production")]
    if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
        router = router
            .route_layer(axum_tracing_opentelemetry::middleware::OtelAxumLayer::default())
            // スパンが作られる**前**に `url.scheme` を補いたいので、この層のさらに外側
            // （後から `route_layer` した層ほど外側 = 先に実行される）に置く。
            .route_layer(middleware::from_fn(crate::middleware::scheme::normalize));
    }

    router.with_state(app_state)
}
