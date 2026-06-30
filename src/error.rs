use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::fmt;

use crate::models::database::table::JsonValueType;
use crate::models::users::UserRole;

/// 認証 (Authentication) や認可 (Authorization) に関する失敗。
#[derive(Debug)]
pub enum AuthError {
    /// `Authorization` ヘッダが存在しない。
    MissingToken,
    /// `Authorization` ヘッダの形式が不正（`Bearer ` で始まらない、非 ASCII 等）。
    MalformedHeader,
    /// JWT の署名検証・期限などに失敗した。
    InvalidToken,
    /// 署名は妥当だが、対応するユーザーが存在しない、または `uid`/`ver` の
    /// 不一致でトークンが失効している。外部にはどちらか区別せず「無効」とだけ返す。
    TokenRevoked,
    /// ログイン時のユーザー名・パスワードが一致しない。
    /// （ユーザーの存在有無を区別せず同一メッセージを返し、ユーザー列挙を防ぐ）
    InvalidCredentials,
    /// GlobalAdmin 権限が必要な操作を、非管理者が要求した。
    RequiresGlobalAdmin,
    /// 本人または GlobalAdmin のみ許可される操作を、第三者が要求した。
    NotSelfOrAdmin,
    /// 対象データベースに対する権限が不足している。
    InsufficientPrivilege { db_name: String, required: UserRole },
    /// root ユーザーに対して許可されない操作（削除・管理者権限変更など）。
    RootProtected,
}

impl AuthError {
    pub fn status(&self) -> StatusCode {
        match self {
            AuthError::MissingToken
            | AuthError::MalformedHeader
            | AuthError::InvalidToken
            | AuthError::TokenRevoked
            | AuthError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AuthError::RequiresGlobalAdmin
            | AuthError::NotSelfOrAdmin
            | AuthError::InsufficientPrivilege { .. }
            | AuthError::RootProtected => StatusCode::FORBIDDEN,
        }
    }

    /// クライアントが分岐に使える安定した機械可読コード。
    pub fn code(&self) -> &'static str {
        match self {
            AuthError::MissingToken => "missing_token",
            AuthError::MalformedHeader => "malformed_header",
            AuthError::InvalidToken => "invalid_token",
            AuthError::TokenRevoked => "token_revoked",
            AuthError::InvalidCredentials => "invalid_credentials",
            AuthError::RequiresGlobalAdmin => "requires_global_admin",
            AuthError::NotSelfOrAdmin => "not_self_or_admin",
            AuthError::InsufficientPrivilege { .. } => "insufficient_privilege",
            AuthError::RootProtected => "root_protected",
        }
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::MissingToken => write!(f, "Missing Authorization header"),
            AuthError::MalformedHeader => write!(f, "Invalid Authorization header format"),
            AuthError::InvalidToken => write!(f, "Invalid or expired token"),
            AuthError::TokenRevoked => write!(f, "Authentication token is no longer valid"),
            AuthError::InvalidCredentials => write!(f, "Invalid username or password"),
            AuthError::RequiresGlobalAdmin => write!(f, "Requires GlobalAdmin privileges"),
            AuthError::NotSelfOrAdmin => write!(f, "You can only modify your own account"),
            AuthError::InsufficientPrivilege { db_name, required } => write!(
                f,
                "Insufficient privileges for database '{}' (requires {:?})",
                db_name, required
            ),
            AuthError::RootProtected => {
                write!(f, "This operation is not allowed on the root user")
            }
        }
    }
}

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    /// 認証・認可に関する失敗（[`AuthError`] を参照）。
    Auth(AuthError),
    InternalError(String),
    Conflict(String),

    DatabaseNotFound {
        name: String,
    },

    DatabaseAlreadyExists {
        name: String,
    },

    TableNotFound {
        name: String,
    },

    TableAlreadyExists {
        name: String,
    },

    ValueTypeMismatch {
        actual: JsonValueType,
        expected: JsonValueType,
    },
    ConstraintViolation {
        reason: String,
    },
    NumericValueOutOfRange {
        actual: String,
        expected: String,
    },
    InvalidStoredValue {
        reason: String,
    },
    StorageError(heed::Error),
    InvalidName {
        reason: String,
    },
    LogicError(kasane_logic::Error),
    ZoomLevelPolicy {
        max_zoom_level: u8,
        input_zoom_level: u8,
    },
}

impl AppError {
    fn status(&self) -> StatusCode {
        match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Auth(e) => e.status(),
            AppError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::DatabaseNotFound { .. } => StatusCode::NOT_FOUND,
            AppError::DatabaseAlreadyExists { .. } => StatusCode::CONFLICT,
            AppError::TableNotFound { .. } => StatusCode::NOT_FOUND,
            AppError::TableAlreadyExists { .. } => StatusCode::CONFLICT,
            AppError::ValueTypeMismatch { .. } => StatusCode::BAD_REQUEST,
            AppError::ConstraintViolation { .. } => StatusCode::BAD_REQUEST,
            AppError::NumericValueOutOfRange { .. } => StatusCode::BAD_REQUEST,
            AppError::InvalidStoredValue { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::StorageError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::InvalidName { .. } => StatusCode::BAD_REQUEST,
            AppError::LogicError(_) => StatusCode::BAD_REQUEST,
            AppError::ZoomLevelPolicy { .. } => StatusCode::BAD_REQUEST,
        }
    }

    /// クライアントが分岐に使える安定した機械可読コード。
    fn code(&self) -> &'static str {
        match self {
            AppError::NotFound(_) => "not_found",
            AppError::Auth(e) => e.code(),
            AppError::InternalError(_) => "internal_error",
            AppError::Conflict(_) => "conflict",
            AppError::DatabaseNotFound { .. } => "database_not_found",
            AppError::DatabaseAlreadyExists { .. } => "database_already_exists",
            AppError::TableNotFound { .. } => "table_not_found",
            AppError::TableAlreadyExists { .. } => "table_already_exists",
            AppError::ValueTypeMismatch { .. } => "value_type_mismatch",
            AppError::ConstraintViolation { .. } => "constraint_violation",
            AppError::NumericValueOutOfRange { .. } => "numeric_value_out_of_range",
            AppError::InvalidStoredValue { .. } => "invalid_stored_value",
            AppError::StorageError(_) => "storage_error",
            AppError::InvalidName { .. } => "invalid_name",
            AppError::LogicError(_) => "logic_error",
            AppError::ZoomLevelPolicy { .. } => "zoom_level_policy",
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "見つかりません: {}", msg),
            AppError::Auth(e) => write!(f, "認証エラー: {}", e),
            AppError::InternalError(msg) => write!(f, "内部エラー: {}", msg),
            AppError::Conflict(msg) => write!(f, "競合エラー: {}", msg),
            AppError::DatabaseNotFound { name } => {
                write!(f, "データベース '{}' が見つかりません", name)
            }
            AppError::DatabaseAlreadyExists { name } => {
                write!(f, "データベース '{}' は既に存在します", name)
            }
            AppError::TableNotFound { name } => write!(f, "テーブル '{}' が見つかりません", name),
            AppError::TableAlreadyExists { name } => {
                write!(f, "テーブル '{}' は既に存在します", name)
            }
            AppError::StorageError(message) => write!(f, "ストレージエラー: {}", message),
            AppError::InvalidName { reason } => write!(f, "無効な名前: {}", reason),
            AppError::LogicError(error) => write!(f, "ロジックエラー: {}", error),
            AppError::ValueTypeMismatch { actual, expected } => write!(
                f,
                "値の型が一致しません: {:?} を想定していましたが、{:?} が指定されました",
                expected, actual
            ),
            AppError::ConstraintViolation { reason } => write!(f, "制約違反: {}", reason),
            AppError::NumericValueOutOfRange { actual, expected } => write!(
                f,
                "数値が範囲外です: {} を想定していましたが、{} が指定されました",
                expected, actual
            ),
            AppError::InvalidStoredValue { reason } => {
                write!(f, "保存された値が無効です: {}", reason)
            }
            AppError::ZoomLevelPolicy {
                max_zoom_level,
                input_zoom_level,
            } => write!(
                f,
                "ズームレベルは {} 以下である必要がありますが、{} が指定されました",
                max_zoom_level, input_zoom_level
            ),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let code = self.code();
        let message = self.to_string();

        let body = Json(json!({ "error": message, "code": code }));
        (status, body).into_response()
    }
}

impl From<AuthError> for AppError {
    fn from(error: AuthError) -> Self {
        AppError::Auth(error)
    }
}

impl From<heed::Error> for AppError {
    fn from(error: heed::Error) -> Self {
        AppError::StorageError(error)
    }
}

impl From<kasane_logic::Error> for AppError {
    fn from(value: kasane_logic::Error) -> Self {
        AppError::LogicError(value)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        AppError::InvalidStoredValue {
            reason: error.to_string(),
        }
    }
}
