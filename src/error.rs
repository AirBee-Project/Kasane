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
#[derive(Debug, Clone)]
pub enum AuthError {
    /// `Authorization` ヘッダが存在しない。
    MissingToken,
    /// `Authorization` ヘッダの形式が不正（`Bearer ` で始まらない、非 ASCII 等）。
    MalformedHeader,
    /// JWT の署名検証・期限などに失敗した。
    InvalidToken,
    /// ユーザーが存在しないのか `uid`/`ver` 不一致なのかは、外部へ区別を返さない。
    TokenRevoked,
    /// ユーザーの存在有無を区別せず同一メッセージを返し、ユーザー列挙を防ぐ。
    InvalidCredentials,
    /// `global` スコープで一定以上のロールが必要な操作を、満たさない利用者が要求した。
    ///
    /// `required` を持つのは、`admin` を求める操作と `manage` で足りる操作を
    /// クライアントが区別できるようにするため。
    RequiresGlobalRole { required: UserRole },
    /// 本人または GlobalAdmin のみ許可される操作を、第三者が要求した。
    NotSelfOrAdmin,
    /// 対象データベース（またはその中の特定テーブル）に対する権限が不足している。
    InsufficientPrivilege {
        db_name: String,
        table_name: Option<String>,
        required: UserRole,
    },
    /// root ユーザーに対して許可されない操作（削除・権限変更など）。
    RootProtected,
}

impl AuthError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::MissingToken
            | Self::MalformedHeader
            | Self::InvalidToken
            | Self::TokenRevoked
            | Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::RequiresGlobalRole { .. }
            | Self::NotSelfOrAdmin
            | Self::InsufficientPrivilege { .. }
            | Self::RootProtected => StatusCode::FORBIDDEN,
        }
    }

    /// クライアントが分岐に使える安定した機械可読コード。
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingToken => "missing_token",
            Self::MalformedHeader => "malformed_header",
            Self::InvalidToken => "invalid_token",
            Self::TokenRevoked => "token_revoked",
            Self::InvalidCredentials => "invalid_credentials",
            // `admin` を求める場合だけ従来のコードを保つ。制御面の操作かどうかは
            // クライアントの分岐で意味が違うので、`manage` で足りる操作と混ぜない。
            Self::RequiresGlobalRole {
                required: UserRole::Admin,
            } => "requires_global_admin",
            Self::RequiresGlobalRole { .. } => "requires_global_role",
            Self::NotSelfOrAdmin => "not_self_or_admin",
            Self::InsufficientPrivilege { .. } => "insufficient_privilege",
            Self::RootProtected => "root_protected",
        }
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingToken => write!(f, "Missing Authorization header"),
            Self::MalformedHeader => write!(f, "Invalid Authorization header format"),
            Self::InvalidToken => write!(f, "Invalid or expired token"),
            Self::TokenRevoked => write!(f, "Authentication token is no longer valid"),
            Self::InvalidCredentials => write!(f, "Invalid username or password"),
            Self::RequiresGlobalRole {
                required: UserRole::Admin,
            } => write!(f, "Requires GlobalAdmin privileges"),
            Self::RequiresGlobalRole { required } => {
                write!(f, "Requires the global '{:?}' role or higher", required)
            }
            Self::NotSelfOrAdmin => write!(f, "You can only modify your own account"),
            Self::InsufficientPrivilege {
                db_name,
                table_name: Some(table_name),
                required,
            } => write!(
                f,
                "Insufficient privileges for table '{}.{}' (requires {:?})",
                db_name, table_name, required
            ),
            Self::InsufficientPrivilege {
                db_name, required, ..
            } => write!(
                f,
                "Insufficient privileges for database '{}' (requires {:?})",
                db_name, required
            ),
            Self::RootProtected => {
                write!(f, "This operation is not allowed on the root user")
            }
        }
    }
}

/// `Clone` なのは、1 つのコミット結果をバッチ全員へ配るため（`coalesce` を参照）。
#[derive(Debug, Clone)]
pub enum AppError {
    NotFound(String),
    /// 認証・認可に関する失敗（[`AuthError`] を参照）。
    Auth(AuthError),
    InternalError(String),
    Conflict(String),
    InvalidSpatialId {
        reason: String,
    },

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
    /// feature で差し替えられるよう、具体的なエラー型は持ち込まずメッセージへ落とす。
    StorageError(String),
    InvalidName {
        reason: String,
    },
    LogicError(kasane_logic::Error),
    ZoomLevelPolicy {
        max_zoom_level: u8,
        input_zoom_level: u8,
    },
    /// 権限ルールの内容が不正（`global` 以外での `admin`、同一対象へのロール矛盾など）。
    InvalidPrivilege {
        reason: String,
    },
}

impl AppError {
    /// 保持できる権限数の上限に達した。付与側と解決側で同じ文言を使う。
    pub fn too_many_privileges() -> Self {
        Self::InvalidPrivilege {
            reason: format!(
                "a user cannot hold more than {} privileges",
                crate::models::users::MAX_PRIVILEGES_PER_USER
            ),
        }
    }

    /// 剥奪しようとした対象の権限を持っていない。
    pub fn no_such_privilege() -> Self {
        Self::NotFound("The user has no privilege for that target".into())
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Auth(e) => e.status(),
            Self::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::InvalidSpatialId { .. } => StatusCode::BAD_REQUEST,
            Self::DatabaseNotFound { .. } => StatusCode::NOT_FOUND,
            Self::DatabaseAlreadyExists { .. } => StatusCode::CONFLICT,
            Self::TableNotFound { .. } => StatusCode::NOT_FOUND,
            Self::TableAlreadyExists { .. } => StatusCode::CONFLICT,
            Self::ValueTypeMismatch { .. } => StatusCode::BAD_REQUEST,
            Self::ConstraintViolation { .. } => StatusCode::BAD_REQUEST,
            Self::NumericValueOutOfRange { .. } => StatusCode::BAD_REQUEST,
            Self::InvalidStoredValue { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::StorageError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidName { .. } => StatusCode::BAD_REQUEST,
            Self::LogicError(_) => StatusCode::BAD_REQUEST,
            Self::ZoomLevelPolicy { .. } => StatusCode::BAD_REQUEST,
            Self::InvalidPrivilege { .. } => StatusCode::BAD_REQUEST,
        }
    }

    /// クライアントが分岐に使える安定した機械可読コード。
    fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::Auth(e) => e.code(),
            Self::InternalError(_) => "internal_error",
            Self::Conflict(_) => "conflict",
            Self::InvalidSpatialId { .. } => "invalid_spatial_id",
            Self::DatabaseNotFound { .. } => "database_not_found",
            Self::DatabaseAlreadyExists { .. } => "database_already_exists",
            Self::TableNotFound { .. } => "table_not_found",
            Self::TableAlreadyExists { .. } => "table_already_exists",
            Self::ValueTypeMismatch { .. } => "value_type_mismatch",
            Self::ConstraintViolation { .. } => "constraint_violation",
            Self::NumericValueOutOfRange { .. } => "numeric_value_out_of_range",
            Self::InvalidStoredValue { .. } => "invalid_stored_value",
            Self::StorageError(_) => "storage_error",
            Self::InvalidName { .. } => "invalid_name",
            Self::LogicError(_) => "logic_error",
            Self::ZoomLevelPolicy { .. } => "zoom_level_policy",
            Self::InvalidPrivilege { .. } => "invalid_privilege",
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::Auth(e) => write!(f, "Authentication error: {}", e),
            Self::InternalError(msg) => write!(f, "Internal error: {}", msg),
            Self::Conflict(msg) => write!(f, "Conflict error: {}", msg),
            Self::InvalidSpatialId { reason } => {
                write!(f, "Invalid Spatial ID: {}", reason)
            }
            Self::DatabaseNotFound { name } => {
                write!(f, "Database '{}' not found", name)
            }
            Self::DatabaseAlreadyExists { name } => {
                write!(f, "Database '{}' already exists", name)
            }
            Self::TableNotFound { name } => write!(f, "Table '{}' not found", name),
            Self::TableAlreadyExists { name } => {
                write!(f, "Table '{}' already exists", name)
            }
            Self::StorageError(message) => write!(f, "Storage error: {}", message),
            Self::InvalidName { reason } => write!(f, "Invalid name: {}", reason),
            Self::LogicError(error) => write!(f, "Logic error: {}", error),
            Self::ValueTypeMismatch { actual, expected } => write!(
                f,
                "Value type mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::ConstraintViolation { reason } => {
                write!(f, "Constraint violation: {}", reason)
            }
            Self::NumericValueOutOfRange { actual, expected } => write!(
                f,
                "Numeric value out of range: expected {}, got {}",
                expected, actual
            ),
            Self::InvalidStoredValue { reason } => {
                write!(f, "Invalid stored value: {}", reason)
            }
            Self::ZoomLevelPolicy {
                max_zoom_level,
                input_zoom_level,
            } => write!(
                f,
                "Zoom level policy violation: expected max {}, got {}",
                max_zoom_level, input_zoom_level
            ),
            Self::InvalidPrivilege { reason } => {
                write!(f, "Invalid privilege rule: {}", reason)
            }
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
        Self::Auth(error)
    }
}

impl From<kasane_logic::Error> for AppError {
    fn from(value: kasane_logic::Error) -> Self {
        Self::LogicError(value)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::InvalidStoredValue {
            reason: error.to_string(),
        }
    }
}
