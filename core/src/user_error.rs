use bcrypt::BcryptError;
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

    #[error("Invalid user password :{reason} (at {location})")]
    UserPasswordVaildationError {
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

    #[error("Range error: {message}")]
    RangeError { message: String },

    #[error("Insert error: failed to insert key '{key_name}' into space '{space_name}'")]
    InsertError {
        space_name: String,
        key_name: String,
    },

    // redb関連のエラーを追加
    #[error("Database error: {message} (at {location})")]
    DatabaseError { message: String, location: String },

    #[error("Database is already open (at {location})")]
    DatabaseAlreadyOpen { location: String },

    #[error("Database is corrupted: {reason} (at {location})")]
    DatabaseCorrupted { reason: String, location: String },

    #[error("Table '{table_name}' does not exist (at {location})")]
    TableDoesNotExist {
        table_name: String,
        location: String,
    },

    #[error("Table '{table_name}' already exists (at {location})")]
    TableAlreadyExists {
        table_name: String,
        location: String,
    },

    #[error("Table type mismatch for '{table_name}': expected key={expected_key}, value={expected_value} (at {location})")]
    TableTypeMismatch {
        table_name: String,
        expected_key: String,
        expected_value: String,
        location: String,
    },

    #[error("Transaction in progress (at {location})")]
    TransactionInProgress { location: String },

    #[error("IO error: {message} (at {location})")]
    IoError { message: String, location: String },

    //bcrypt関連
    #[error("Password error: {message} (at {location})")]
    PasswordError { message: String, location: String },

    #[error("Password too long: {length} bytes (max 72 bytes) (at {location})")]
    PasswordTooLong { length: usize, location: String },
}

// 主要なエラー型は詳細に処理
impl From<redb::Error> for UserError {
    fn from(err: redb::Error) -> Self {
        let location = format!("{}:{}", file!(), line!());
        match err {
            redb::Error::TableDoesNotExist(table_name) => UserError::TableDoesNotExist {
                table_name,
                location,
            },
            redb::Error::Corrupted(reason) => UserError::DatabaseCorrupted { reason, location },
            _ => UserError::DatabaseError {
                message: err.to_string(),
                location,
            },
        }
    }
}

// その他のエラー型は汎用的に処理
macro_rules! impl_from_redb_errors {
    ($($error_type:ty),+ $(,)?) => {
        $(
            impl From<$error_type> for UserError {
                fn from(err: $error_type) -> Self {
                    let location = format!("{}:{}", file!(), line!());
                    UserError::DatabaseError {
                        message: err.to_string(),
                        location,
                    }
                }
            }
        )+
    };
}

impl_from_redb_errors!(
    redb::TransactionError,
    redb::StorageError,
    redb::CommitError,
    redb::TableError,
);

impl From<BcryptError> for UserError {
    fn from(err: BcryptError) -> Self {
        let location = format!("{}:{}", file!(), line!());

        match err {
            BcryptError::Io(io_err) => UserError::IoError {
                message: format!("Password hashing IO error: {}", io_err),
                location,
            },
            BcryptError::Truncation(len) => UserError::PasswordTooLong {
                length: len,
                location,
            },
            other => UserError::PasswordError {
                message: other.to_string(),
                location,
            },
        }
    }
}
