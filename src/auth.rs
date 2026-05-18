//! APIキー認証を提供するモジュール。
//!
//! このモジュールは、リクエストヘッダーから認証キーを抽出し、
//! 設定された読み取り用キー（`READ_KEY`）または書き込み用キー（`WRITE_KEY`）と照合して
//! アクセスを制限する Axum の Extractor ガードを提供します。

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::AppState;

/// リクエストヘッダー（`Authorization` または `x-api-key`）から提供されたAPIキーを抽出します。
fn get_provided_key(parts: &Parts) -> Option<&str> {
    let auth_header = parts
        .headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            let (scheme, credentials) = value.split_once(' ')?;
            scheme.eq_ignore_ascii_case("Bearer").then_some(credentials)
        });

    let x_api_key = parts
        .headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok());

    auth_header.or(x_api_key)
}

/// 認証エラー時に返却する `401 Unauthorized` レスポンスを生成します。
fn unauthorized_response() -> Response {
    let error_response = json!({
        "message": "Unauthorized"
    });
    (StatusCode::UNAUTHORIZED, Json(error_response)).into_response()
}

/// 読み取り操作（READ）に対する認証ガード。
///
/// `AppState` に `read_key` が設定されている場合、提供されたキーが `read_key`
/// もしくは `write_key` のいずれかに一致することを確認します。
/// キーが設定されていない場合は、認証なしで通過を許可します。
pub struct RequireRead;

impl FromRequestParts<AppState> for RequireRead {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(read_key) = &state.read_key {
            let provided_key = get_provided_key(parts);
            if let Some(key) = provided_key
                && (key == read_key || state.write_key.as_deref() == Some(key))
            {
                return Ok(RequireRead);
            }
            return Err(unauthorized_response());
        }
        Ok(RequireRead)
    }
}

/// 書き込み操作（WRITE）に対する認証ガード。
///
/// `AppState` に `write_key` が設定されている場合、提供されたキーが `write_key`
/// に完全に一致することを確認します。
/// キーが設定されていない場合は、認証なしで通過を許可します。
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
                && key == write_key
            {
                return Ok(RequireWrite);
            }
            return Err(unauthorized_response());
        }
        Ok(RequireWrite)
    }
}
