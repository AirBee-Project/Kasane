// src/routes/mod.rs
use axum::{
    Router, middleware,
    routing::{delete, get, post, put},
};

use crate::AppState;
mod database;
mod openapi;

use tower_http::trace::TraceLayer;

pub fn create_router(app_state: AppState) -> Router {
    let protected_router = Router::new()
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
        .nest("/databases", database::routes())
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            crate::middleware::auth::require_auth,
        ));

    let auth_router = Router::new().route("/auth/login", post(crate::handlers::auth::login));

    Router::new()
        .merge(auth_router)
        .merge(protected_router)
        .merge(openapi::routes())
        .layer(TraceLayer::new_for_http())
        .with_state(app_state)
}
