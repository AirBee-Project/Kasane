use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Error {
    // Name validation errors with context
    SpaceNameValidationError {
        name: String,
        reason: &'static str,
        location: &'static str,
    },
    KeyNameValidationError {
        name: String,
        reason: &'static str,
        location: &'static str,
    },

    UserAlreadyExists {
        user_name: String,
    },

    UserNotFound {
        user_name: String,
    },

    // Parse errors with context
    ParseError {
        message: String,
        location: &'static str,
    },

    // Resource existence errors
    SpaceAlreadyExists {
        space_name: String,
    },
    SpaceNotFound {
        space_name: String,
    },
    KeyNotFound {
        key_name: String,
        space_name: String,
        location: &'static str,
    },
    KeyAlreadyExists {
        key_name: String,
        space_name: String,
        location: &'static str,
    },
    ValueAlreadyExists {
        space_time_id: String,
        location: &'static str,
    },
    TypeMismatchFilter {
        expected_type: String,
        operation: String,
        location: &'static str,
    },
    TypeMismatchValue {
        expected_type: String,
        received_type: String,
        location: &'static str,
    },
    QueueSendError {
        location: &'static str,
    },
    QueueReceiveError {
        location: &'static str,
    },
    QueueFull {
        location: &'static str,
    },
    LmdbError {
        message: String,
        location: &'static str,
    },
    LmdbMapFull {
        attempted_size: usize,
        location: &'static str,
    },
    LmdbTxnError {
        message: &'static str,
        location: &'static str,
    },
    LmdbDbNotFound {
        db_name: &'static str,
        location: &'static str,
    },
    RangeError {
        message: String,
    },
    InsertError {
        space_name: String,
        key_name: String,
    },
    NnKnown,
}

use std::fmt;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::SpaceNameValidationError {
                name,
                reason,
                location,
            } => {
                write!(
                    f,
                    "Invalid space name '{}': {} (at {})",
                    name, reason, location
                )
            }
            Error::KeyNameValidationError {
                name,
                reason,
                location,
            } => {
                write!(
                    f,
                    "Invalid key name '{}': {} (at {})",
                    name, reason, location
                )
            }
            Error::ParseError { message, location } => {
                write!(f, "Parse error: {} (at {})", message, location)
            }
            Error::SpaceAlreadyExists { space_name } => {
                write!(f, "Space '{}' already exists (at)", space_name,)
            }
            Error::SpaceNotFound { space_name } => {
                write!(f, "Space '{}' not found (at)", space_name)
            }
            Error::KeyNotFound {
                key_name,
                space_name,
                location,
            } => {
                write!(
                    f,
                    "Key '{}' not found in space '{}' (at {})",
                    key_name, space_name, location
                )
            }
            Error::KeyAlreadyExists {
                key_name,
                space_name,
                location,
            } => {
                write!(
                    f,
                    "Key '{}' already exists in space '{}' (at {})",
                    key_name, space_name, location
                )
            }
            Error::ValueAlreadyExists {
                space_time_id,
                location,
            } => {
                write!(
                    f,
                    "Value already exists for SpaceTimeId '{}' (at {})",
                    space_time_id, location
                )
            }
            Error::TypeMismatchFilter {
                expected_type,
                operation,
                location,
            } => {
                write!(
                    f,
                    "Type mismatch: expected '{}' type for {} operation (at {})",
                    expected_type, operation, location
                )
            }
            Error::TypeMismatchValue {
                expected_type,
                received_type,
                location,
            } => {
                write!(
                    f,
                    "Type mismatch: expected '{}' but received '{}' (at {})",
                    expected_type, received_type, location
                )
            }
            Error::QueueSendError { location } => {
                write!(f, "Failed to send job to queue (at {})", location)
            }
            Error::QueueReceiveError { location } => {
                write!(f, "Failed to receive job from queue (at {})", location)
            }
            Error::QueueFull { location } => {
                write!(f, "Queue is full, cannot enqueue job (at {})", location)
            }
            Error::LmdbError { message, location } => {
                write!(f, "LMDB error: {} (at {})", message, location)
            }
            Error::LmdbMapFull {
                attempted_size,
                location,
            } => {
                write!(
                    f,
                    "LMDB map full: attempted size {} bytes (at {})",
                    attempted_size, location
                )
            }
            Error::LmdbTxnError { message, location } => {
                write!(f, "LMDB transaction error: {} (at {})", message, location)
            }
            Error::LmdbDbNotFound { db_name, location } => {
                write!(f, "LMDB database '{}' not found (at {})", db_name, location)
            }
            // 他の既存バリアントは省略
            _ => write!(f, "Other error"),
        }
    }
}
