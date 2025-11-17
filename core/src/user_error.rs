use bcrypt::BcryptError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum UserError {
    // ==================== Validation Errors ====================
    // バリデーションエラー: 名前やパスワードの検証で使用
    
    #[error("Invalid space name '{name}': {reason} (at {location})")]
    SpaceNameValidationError {
        name: String,
        reason: &'static str,
        location: String,
    },

    #[error("Invalid key name '{name}': {reason} (at {location})")]
    KeyNameValidationError {
        name: String,
        reason: &'static str,
        location: String,
    },

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

    #[error("Parse error: {message} (at {location})")]
    ParseError { message: String, location: String },

    // ==================== Entity Not Found Errors ====================
    // エンティティが存在しないエラー
    
    #[error("User '{user_name}' not found")]
    UserNotFound { user_name: String },

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

    // ==================== Entity Already Exists Errors ====================
    // エンティティが既に存在するエラー
    
    #[error("User '{user_name}' already exists (at {location})")]
    UserAlreadyExists { user_name: String, location: String },

    #[error("Space '{space_name}' already exists (at {location})")]
    SpaceAlreadyExists {
        space_name: String,
        location: String,
    },

    #[error("Key '{key_name}' already exists in space '{space_name}' (at {location})")]
    KeyAlreadyExists {
        key_name: String,
        space_name: String,
        location: String,
    },

    // ==================== Authentication & Session Errors ====================
    // 認証とセッション関連のエラー
    
    #[error("Username or password is missing")]
    UserNameOrPasswordMissing,

    #[error("Password error: {message} (at {location})")]
    PasswordError { message: String, location: String },

    #[error("Password too long: {length} bytes (max 72 bytes) (at {location})")]
    PasswordTooLong { length: usize, location: String },

    #[error("Session error: {message} (at {location})")]
    SessionError { message: String, location: String },

    // ==================== Database Errors ====================
    // データベース関連のエラー
    
    #[error("Database error: {message} (at {location})")]
    DatabaseError { message: String, location: String },

    #[error("Database is corrupted: {reason} (at {location})")]
    DatabaseCorrupted { reason: String, location: String },

    #[error("Table '{table_name}' does not exist (at {location})")]
    TableDoesNotExist {
        table_name: String,
        location: String,
    },

    // ==================== System/IO Errors ====================
    // システムとI/O関連のエラー
    
    #[error("IO error: {message} (at {location})")]
    IoError { message: String, location: String },

    // ==================== Queue Errors ====================
    // キュー関連のエラー
    
    #[error("Failed to send job to queue (at {location})")]
    QueueSendError { location: String },

    #[error("Failed to receive job from queue (at {location})")]
    QueueReceiveError { location: String },
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
    redb::DatabaseError,
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
