use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum UserError {
    // CreateSpace
    #[error("Invalid space name '{name}': {reason} (at {location})")]
    SpaceNameValidationError {
        name: String,
        reason: &'static str,
        location: String,
    },

    //CreateKey
    #[error("Invalid key name '{name}': {reason} (at {location})")]
    KeyNameValidationError {
        name: String,
        reason: &'static str,
        location: String,
    },

    //CreateUser
    #[error("Invalid user name '{name}': {reason} (at {location})")]
    UserNameVaildationError {
        name: String,
        reason: &'static str,
        location: String,
    },

    #[error("User '{user_name}' already exists (at {location})")]
    UserAlreadyExists { user_name: String, location: String },

    #[error("User '{user_name}' not found")]
    UserNotFound { user_name: String },

    #[error("Parse error: {message} (at {location})")]
    ParseError { message: String, location: String },

    #[error("Space '{space_name}' already exists (at {location})")]
    SpaceAlreadyExists {
        space_name: String,
        location: String,
    },

    #[error("Space '{space_name}' not found (at {location})")]
    SpaceNotFound {
        space_name: String,
        location: String,
    },

    #[error("Key '{key_name}' not found in space '{space_name}' (at {location})")]
    KeyNotFound {
        key_name: String,
        space_name: String,
        location: String,
    },

    #[error("Key '{key_name}' already exists in space '{space_name}' (at {location})")]
    KeyAlreadyExists {
        key_name: String,
        space_name: String,
        location: String,
    },

    #[error("Value already exists for SpaceTimeId '{space_time_id}' (at {location})")]
    ValueAlreadyExists {
        space_time_id: String,
        location: String,
    },

    #[error(
        "Type mismatch: expected '{expected_type}' type for {operation} operation (at {location})"
    )]
    TypeMismatchFilter {
        expected_type: String,
        operation: String,
        location: String,
    },

    #[error(
        "Type mismatch: expected '{expected_type}' but received '{received_type}' (at {location})"
    )]
    TypeMismatchValue {
        expected_type: String,
        received_type: String,
        location: String,
    },

    #[error("Failed to send job to queue (at {location})")]
    QueueSendError { location: String },

    #[error("Failed to receive job from queue (at {location})")]
    QueueReceiveError { location: String },

    #[error("Queue is full, cannot enqueue job (at {location})")]
    QueueFull { location: String },

    #[error("LMDB error: {message} (at {location})")]
    LmdbError { message: String, location: String },

    #[error("LMDB map full: attempted size {attempted_size} bytes (at {location})")]
    LmdbMapFull {
        attempted_size: usize,
        location: String,
    },

    #[error("LMDB transaction error: {message} (at {location})")]
    LmdbTxnError {
        message: &'static str,
        location: String,
    },

    #[error("LMDB database '{db_name}' not found (at {location})")]
    LmdbDbNotFound {
        db_name: &'static str,
        location: String,
    },

    #[error("Range error: {message}")]
    RangeError { message: String },

    #[error("Insert error: failed to insert key '{key_name}' into space '{space_name}'")]
    InsertError {
        space_name: String,
        key_name: String,
    },

    #[error("Unknown error {message} (at {location})")]
    UnKnown { message: String, location: String },
}
