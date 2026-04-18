use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    TableNotFound { name: String },
}
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::TableNotFound { name } => write!(f, "Table '{}' not found", name),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // エラーの種類に応じて、ステータスコードとメッセージを決定
        let (status, error_message) = match self {
            AppError::TableNotFound { name } => {
                (StatusCode::NOT_FOUND, format!("Table '{}' not found", name))
            }
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
