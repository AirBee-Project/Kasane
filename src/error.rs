//! エラーの語彙。
//!
//! 500 になる失敗は原因で 3 つに分ける。混ぜると、運用時に「直すべき場所」が判らなくなる。
//!
//! - [`AppError::StorageError`] — ストレージエンジン自身が失敗した（I/O、競合、接続）。
//! - [`AppError::Corrupt`] — エンジンはバイト列を返したが、書いたときの形式と違う。
//! - [`AppError::InternalError`] — このプログラムの不変条件が破れた（バグ）。
//!
//! **文言はすべて英語で書く。** バックエンドごとに別の言い回しを足さないよう、
//! 資源名と壊れた対象は [`Resource`] / [`Stored`] の語彙から組み立てる。

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::fmt;

use crate::models::database::table::JsonValueType;
use crate::models::users::UserRole;

/// エラーが指す資源の種類。
///
/// 「見つからない」「既にある」はどの資源でも同じ形なので、資源ごとに変種を作らず
/// これで区別する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    Database,
    Table,
    User,
}

impl Resource {
    pub fn not_found(self, name: impl Into<String>) -> AppError {
        AppError::NotFound {
            resource: self,
            name: name.into(),
        }
    }

    pub fn already_exists(self, name: impl Into<String>) -> AppError {
        AppError::AlreadyExists {
            resource: self,
            name: name.into(),
        }
    }

    /// `(見つからない, 既にある)` の機械可読コード。
    ///
    /// 資源名から組み立てると `&'static str` にできないので、対にして 1 箇所で持つ。
    const fn codes(self) -> (&'static str, &'static str) {
        match self {
            Self::Database => ("database_not_found", "database_already_exists"),
            Self::Table => ("table_not_found", "table_already_exists"),
            Self::User => ("user_not_found", "user_already_exists"),
        }
    }
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Database => "Database",
            Self::Table => "Table",
            Self::User => "User",
        })
    }
}

/// 読み出したバイト列が壊れていた対象。
///
/// **両バックエンドで同じ語彙を使う。** その場で文言を書くと、同じ壊れ方が実装ごとに
/// 違うメッセージになる。細かい壊れ方は [`AppError::Corrupt`] の `detail` が持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    /// 利用者レコード（鍵・値のどちらも）。
    UserRecord,
    /// データベースのメタデータと ID 逆引き。
    DatabaseEntry,
    /// テーブルのメタデータと ID 逆引き。
    TableEntry,
    /// ACL の行（鍵・ロールバイトのどちらも）。
    AclRow,
    /// シャードの鍵・本体・件数。
    Shard,
    /// 値インデックスの鍵。
    ValueIndex,
    /// 回収待ち行列の項目。
    Garbage,
    /// 格納された値を、テーブルが宣言した型として読めなかった。
    Value,
}

impl fmt::Display for Stored {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UserRecord => "user record",
            Self::DatabaseEntry => "database entry",
            Self::TableEntry => "table entry",
            Self::AclRow => "acl row",
            Self::Shard => "shard",
            Self::ValueIndex => "value index entry",
            Self::Garbage => "garbage entry",
            Self::Value => "stored value",
        })
    }
}

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
            // `admin` を求める場合だけ別のコードにする。制御面の操作かどうかは
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
                write!(f, "Requires the global '{required:?}' role or higher")
            }
            Self::NotSelfOrAdmin => write!(f, "You can only modify your own account"),
            Self::InsufficientPrivilege {
                db_name,
                table_name: Some(table_name),
                required,
            } => write!(
                f,
                "Insufficient privileges for table '{db_name}.{table_name}' (requires {required:?})"
            ),
            Self::InsufficientPrivilege {
                db_name, required, ..
            } => write!(
                f,
                "Insufficient privileges for database '{db_name}' (requires {required:?})"
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
    /// 認証・認可に関する失敗（[`AuthError`] を参照）。
    Auth(AuthError),

    // --- 要求が通らない（4xx） ---
    NotFound {
        resource: Resource,
        name: String,
    },
    AlreadyExists {
        resource: Resource,
        name: String,
    },
    /// 剥奪しようとした対象の権限を、その利用者が持っていない。
    ///
    /// 対象はリクエストのパスにあるので、ここには載せない。
    PrivilegeNotFound,
    /// 権限ルールの内容が不正（同一対象へのロール矛盾、保持数の上限超過など）。
    InvalidPrivilege {
        reason: String,
    },
    InvalidName {
        reason: String,
    },
    InvalidSpatialId {
        reason: String,
    },
    ValueTypeMismatch {
        actual: JsonValueType,
        expected: JsonValueType,
    },
    NumericValueOutOfRange {
        actual: String,
        expected: String,
    },
    ConstraintViolation {
        reason: String,
    },
    ZoomLevelPolicy {
        max_zoom_level: u8,
        input_zoom_level: u8,
    },
    LogicError(kasane_logic::Error),
    /// 同時実行または要求同士の食い違いで通せなかった。
    Conflict(String),

    // --- サーバー側の失敗（5xx）。原因で 3 つに分ける（モジュールの説明を参照） ---
    /// ディスク形式の版がこのビルドと合わない。`found` が `None` なら版を持たない世代。
    SchemaVersionMismatch {
        found: Option<u32>,
        expected: u32,
    },
    /// 保存されていたバイト列が読めない。
    Corrupt {
        stored: Stored,
        detail: String,
    },
    /// ストレージエンジン自身が失敗した。
    ///
    /// feature で差し替えられるよう、具体的なエラー型は持ち込まずメッセージへ落とす。
    StorageError(String),
    /// このプログラムの不変条件が破れた（バグ）。
    InternalError(String),
}

impl AppError {
    /// 保存されていたものが読めない。両バックエンドはこれだけを使う。
    pub fn corrupt(stored: Stored, detail: impl fmt::Display) -> Self {
        Self::Corrupt {
            stored,
            detail: detail.to_string(),
        }
    }

    /// 保持できる権限数の上限に達した。付与側と解決側で同じ文言を使う。
    pub fn too_many_privileges() -> Self {
        Self::InvalidPrivilege {
            reason: format!(
                "a user cannot hold more than {} privileges",
                crate::models::users::MAX_PRIVILEGES_PER_USER
            ),
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::Auth(e) => e.status(),
            Self::NotFound { .. } | Self::PrivilegeNotFound => StatusCode::NOT_FOUND,
            Self::AlreadyExists { .. } | Self::Conflict(_) => StatusCode::CONFLICT,
            Self::InvalidPrivilege { .. }
            | Self::InvalidName { .. }
            | Self::InvalidSpatialId { .. }
            | Self::ValueTypeMismatch { .. }
            | Self::NumericValueOutOfRange { .. }
            | Self::ConstraintViolation { .. }
            | Self::ZoomLevelPolicy { .. }
            | Self::LogicError(_) => StatusCode::BAD_REQUEST,
            Self::SchemaVersionMismatch { .. }
            | Self::Corrupt { .. }
            | Self::StorageError(_)
            | Self::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// クライアントが分岐に使える安定した機械可読コード。
    fn code(&self) -> &'static str {
        match self {
            Self::Auth(e) => e.code(),
            Self::NotFound { resource, .. } => resource.codes().0,
            Self::AlreadyExists { resource, .. } => resource.codes().1,
            Self::PrivilegeNotFound => "privilege_not_found",
            Self::InvalidPrivilege { .. } => "invalid_privilege",
            Self::InvalidName { .. } => "invalid_name",
            Self::InvalidSpatialId { .. } => "invalid_spatial_id",
            Self::ValueTypeMismatch { .. } => "value_type_mismatch",
            Self::NumericValueOutOfRange { .. } => "numeric_value_out_of_range",
            Self::ConstraintViolation { .. } => "constraint_violation",
            Self::ZoomLevelPolicy { .. } => "zoom_level_policy",
            Self::LogicError(_) => "logic_error",
            Self::Conflict(_) => "conflict",
            Self::SchemaVersionMismatch { .. } => "schema_version_mismatch",
            Self::Corrupt { .. } => "corrupt_storage",
            Self::StorageError(_) => "storage_error",
            Self::InternalError(_) => "internal_error",
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auth(e) => write!(f, "Authentication error: {e}"),
            Self::NotFound { resource, name } => write!(f, "{resource} '{name}' not found"),
            Self::AlreadyExists { resource, name } => {
                write!(f, "{resource} '{name}' already exists")
            }
            Self::PrivilegeNotFound => {
                write!(f, "The user holds no privilege for that target")
            }
            Self::InvalidPrivilege { reason } => write!(f, "Invalid privilege rule: {reason}"),
            Self::InvalidName { reason } => write!(f, "Invalid name: {reason}"),
            Self::InvalidSpatialId { reason } => write!(f, "Invalid Spatial ID: {reason}"),
            Self::ValueTypeMismatch { actual, expected } => {
                write!(
                    f,
                    "Value type mismatch: expected {expected:?}, got {actual:?}"
                )
            }
            Self::NumericValueOutOfRange { actual, expected } => {
                write!(
                    f,
                    "Numeric value out of range: expected {expected}, got {actual}"
                )
            }
            Self::ConstraintViolation { reason } => write!(f, "Constraint violation: {reason}"),
            Self::ZoomLevelPolicy {
                max_zoom_level,
                input_zoom_level,
            } => write!(
                f,
                "Zoom level policy violation: expected max {max_zoom_level}, got {input_zoom_level}"
            ),
            Self::LogicError(error) => write!(f, "Logic error: {error}"),
            Self::Conflict(msg) => write!(f, "Conflict: {msg}"),
            Self::SchemaVersionMismatch {
                found: Some(found),
                expected,
            } => write!(
                f,
                "On-disk schema version {found} is not supported by this build \
                 (it reads version {expected}); point the server at a fresh location"
            ),
            Self::SchemaVersionMismatch {
                found: None,
                expected,
            } => write!(
                f,
                "On-disk data predates schema versioning and is not supported by this build \
                 (it reads version {expected}); point the server at a fresh location"
            ),
            Self::Corrupt { stored, detail } => write!(f, "Corrupt {stored}: {detail}"),
            Self::StorageError(msg) => write!(f, "Storage error: {msg}"),
            Self::InternalError(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = Json(json!({ "error": self.to_string(), "code": self.code() }));
        (self.status(), body).into_response()
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
