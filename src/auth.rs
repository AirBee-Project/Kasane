use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::AppState;

fn get_provided_key(parts: &Parts) -> Option<&str> {
    let auth_header = parts
        .headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    let x_api_key = parts
        .headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok());

    auth_header.or(x_api_key)
}

fn unauthorized_response() -> Response {
    let error_response = json!({
        "message": "Unauthorized"
    });
    (StatusCode::UNAUTHORIZED, Json(error_response)).into_response()
}

pub struct RequireRead;

impl FromRequestParts<AppState> for RequireRead {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(readonly_key) = &state.readonly_key {
            let provided_key = get_provided_key(parts);
            if let Some(key) = provided_key
                && (key == readonly_key || state.write_key.as_deref() == Some(key)) {
                    return Ok(RequireRead);
                }
            return Err(unauthorized_response());
        }
        Ok(RequireRead)
    }
}

pub struct RequireWrite;

impl FromRequestParts<AppState> for RequireWrite {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(write_key) = &state.write_key {
            let provided_key = get_provided_key(parts);
            if let Some(key) = provided_key
                && key == write_key {
                    return Ok(RequireWrite);
                }
            return Err(unauthorized_response());
        }
        Ok(RequireWrite)
    }
}
