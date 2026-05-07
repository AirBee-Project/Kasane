use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::fmt;

use crate::models::layer::JsonValueType;

#[derive(Debug)]
pub enum AppError {
    LayerNotFound {
        name: String,
    },
    LayerAlreadyExists {
        name: String,
    },
    ValueTypeMismatch {
        actual: JsonValueType,
        expected: JsonValueType,
    },
    NumericValueOutOfRange {
        actual: String,
        expected: String,
    },
    InvalidStoredValue {
        reason: String,
    },
    StorageError(redb::Error),
    InvalidName {
        reason: String,
    },
    LogicError(kasane_logic::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::LayerNotFound { name } => write!(f, "Layer '{}' not found", name),
            AppError::LayerAlreadyExists { name } => write!(f, "Layer '{}' already exists", name),
            AppError::StorageError(message) => write!(f, "{}", message),
            AppError::InvalidName { reason } => write!(f, "Invalid name: {}", reason),
            AppError::LogicError(error) => write!(f, "Logic Error: {}", error),
            AppError::ValueTypeMismatch { actual, expected } => write!(
                f,
                "Value type mismatch: expected {:?}, but got {:?}",
                expected, actual
            ),
            AppError::NumericValueOutOfRange { actual, expected } => write!(
                f,
                "Numeric value out of range: expected {}, but got {}",
                expected, actual
            ),
            AppError::InvalidStoredValue { reason } => {
                write!(f, "Invalid stored value: {}", reason)
            }
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::LayerNotFound { name } => {
                (StatusCode::NOT_FOUND, format!("Layer '{}' not found", name))
            }
            AppError::LayerAlreadyExists { name } => (
                StatusCode::CONFLICT,
                format!("Layer '{}' already exists", name),
            ),
            AppError::StorageError(message) => {
                (StatusCode::INTERNAL_SERVER_ERROR, message.to_string())
            }
            AppError::InvalidName { reason } => {
                (StatusCode::BAD_REQUEST, format!("Invalid name: {}", reason))
            }
            AppError::LogicError(error) => {
                (StatusCode::BAD_REQUEST, format!("Logic Error: {}", error))
            }
            AppError::ValueTypeMismatch { actual, expected } => (
                StatusCode::BAD_REQUEST,
                format!(
                    "Value type mismatch: expected {:?}, but got {:?}",
                    expected, actual
                ),
            ),
            AppError::NumericValueOutOfRange { actual, expected } => (
                StatusCode::BAD_REQUEST,
                format!(
                    "Numeric value out of range: expected {}, but got {}",
                    expected, actual
                ),
            ),
            AppError::InvalidStoredValue { reason } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Invalid stored value: {}", reason),
            ),
        };

        let body = Json(json!({ "error": error_message }));
        (status, body).into_response()
    }
}

impl From<redb::Error> for AppError {
    fn from(error: redb::Error) -> Self { AppError::StorageError(error) }
}
impl From<redb::TransactionError> for AppError {
    fn from(error: redb::TransactionError) -> Self { AppError::StorageError(error.into()) }
}
impl From<redb::DatabaseError> for AppError {
    fn from(error: redb::DatabaseError) -> Self { AppError::StorageError(error.into()) }
}
impl From<redb::TableError> for AppError {
    fn from(error: redb::TableError) -> Self { AppError::StorageError(error.into()) }
}
impl From<redb::StorageError> for AppError {
    fn from(error: redb::StorageError) -> Self { AppError::StorageError(error.into()) }
}
impl From<redb::SavepointError> for AppError {
    fn from(error: redb::SavepointError) -> Self { AppError::StorageError(error.into()) }
}
impl From<redb::CommitError> for AppError {
    fn from(error: redb::CommitError) -> Self { AppError::StorageError(error.into()) }
}
impl From<redb::CompactionError> for AppError {
    fn from(error: redb::CompactionError) -> Self { AppError::StorageError(error.into()) }
}
impl From<redb::SetDurabilityError> for AppError {
    fn from(error: redb::SetDurabilityError) -> Self { AppError::StorageError(error.into()) }
}
impl From<kasane_logic::Error> for AppError {
    fn from(value: kasane_logic::Error) -> Self { AppError::LogicError(value) }
}
