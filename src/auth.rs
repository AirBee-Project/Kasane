use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::AppState;

pub async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let method = req.method().clone();

    let is_readonly = matches!(method, Method::GET | Method::HEAD | Method::OPTIONS);

    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    let x_api_key = req
        .headers()
        .get("x-api-key")
        .and_then(|value| value.to_str().ok());

    let provided_key = auth_header.or(x_api_key);

    let is_authorized = if is_readonly {
        if let Some(readonly_key) = &state.readonly_key {
            if let Some(key) = provided_key {
                key == readonly_key || state.write_key.as_deref() == Some(key)
            } else {
                false
            }
        } else {
            true
        }
    } else {
        if let Some(write_key) = &state.write_key {
            if let Some(key) = provided_key {
                key == write_key
            } else {
                false
            }
        } else {
            true
        }
    };

    if !is_authorized {
        let error_response = json!({
            "message": "Unauthorized"
        });
        return (StatusCode::UNAUTHORIZED, Json(error_response)).into_response();
    }

    next.run(req).await
}
