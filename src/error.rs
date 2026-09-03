use crate::models::database::table::JsonValueType;
use crate::models::users::UserRole;
use std::fmt;

/// エラーが指すリソースの種類
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Resource {
    #[error("Database")]
    Database,
    #[error("Table")]
    Table,
    #[error("User")]
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

    /// 代表的なリソースに対する `(見つからない, 既にある)` の語句
    const fn codes(self) -> (&'static str, &'static str) {
        match self {
            Self::Database => ("database_not_found", "database_already_exists"),
            Self::Table => ("table_not_found", "table_already_exists"),
            Self::User => ("user_not_found", "user_already_exists"),
        }
    }
}

/// 読み出したバイト列が壊れていた対象。
///
/// **両バックエンドで同じ語彙を使う。** その場で文言を書くと、同じ壊れ方が実装ごとに
/// 違うメッセージになる。細かい壊れ方は [`AppError::Corrupt`] の `detail` が持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Stored {
    /// 利用者レコード（鍵・値のどちらも）。
    #[error("user record")]
    UserRecord,
    /// データベースのメタデータと ID 逆引き。
    #[error("database entry")]
    DatabaseEntry,
    /// テーブルのメタデータと ID 逆引き。
    #[error("table entry")]
    TableEntry,
    /// ACL の行（鍵・ロールバイトのどちらも）。
    #[error("acl row")]
    AclRow,
    /// シャードの鍵・本体・件数。
    #[error("shard")]
    Shard,
    /// 値インデックスの鍵。
    #[error("value index entry")]
    ValueIndex,
    /// 回収待ち行列の項目。
    #[error("garbage entry")]
    Garbage,
    /// 格納された値を、テーブルが宣言した型として読めなかった。
    #[error("stored value")]
    Value,
}

/// 認証 (Authentication) や認可 (Authorization) に関する失敗。
#[derive(Debug, Clone, thiserror::Error)]
pub enum AuthError {
    /// `Authorization` ヘッダが存在しない。
    #[error("Missing Authorization header")]
    MissingToken,
    /// `Authorization` ヘッダの形式が不正（`Bearer ` で始まらない、非 ASCII 等）。
    #[error("Invalid Authorization header format")]
    MalformedHeader,
    /// JWT の署名検証・期限などに失敗した。
    #[error("Invalid or expired token")]
    InvalidToken,
    /// ユーザーが存在しないのか `uid`/`ver` 不一致なのかは、外部へ区別を返さない。
    #[error("Authentication token is no longer valid")]
    TokenRevoked,
    /// ユーザーの存在有無を区別せず同一メッセージを返し、ユーザー列挙を防ぐ。
    #[error("Invalid username or password")]
    InvalidCredentials,
    /// `global` スコープで一定以上のロールが必要な操作を、満たさない利用者が要求した。
    ///
    /// `required` を持つのは、`admin` を求める操作と `manage` で足りる操作を
    /// クライアントが区別できるようにするため。
    #[error("{}", match required { UserRole::Admin => "Requires GlobalAdmin privileges".to_string(), req => format!("Requires the global '{req:?}' role or higher") })]
    RequiresGlobalRole { required: UserRole },
    /// 本人または GlobalAdmin のみ許可される操作を、第三者が要求した。
    #[error("You can only modify your own account")]
    NotSelfOrAdmin,
    /// 対象データベース（またはその中の特定テーブル）に対する権限が不足している。
    #[error("{}", match table_name { Some(t) => format!("Insufficient privileges for table '{db_name}.{t}' (requires {required:?})"), None => format!("Insufficient privileges for database '{db_name}' (requires {required:?})") })]
    InsufficientPrivilege {
        db_name: String,
        table_name: Option<String>,
        required: UserRole,
    },
    /// root ユーザーに対して許可されない操作（削除・権限変更など）。
    #[error("This operation is not allowed on the root user")]
    RootProtected,
}

impl AuthError {
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

    fn grpc_code(&self) -> tonic::Code {
        match self {
            Self::MissingToken
            | Self::MalformedHeader
            | Self::InvalidToken
            | Self::TokenRevoked
            | Self::InvalidCredentials => tonic::Code::Unauthenticated,
            Self::RequiresGlobalRole { .. }
            | Self::NotSelfOrAdmin
            | Self::InsufficientPrivilege { .. }
            | Self::RootProtected => tonic::Code::PermissionDenied,
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    /// 認証・認可に関する失敗（[`AuthError`] を参照）。
    #[error("Authentication error: {0}")]
    Auth(#[from] AuthError),

    // --- 要求が通らない（4xx） ---
    #[error("{resource} '{name}' not found")]
    NotFound { resource: Resource, name: String },
    #[error("{resource} '{name}' already exists")]
    AlreadyExists { resource: Resource, name: String },
    /// 剥奪しようとした対象の権限を、その利用者が持っていない。
    ///
    /// 対象はリクエストのパスにあるので、ここには載せない。
    #[error("The user holds no privilege for that target")]
    PrivilegeNotFound,
    /// 権限ルールの内容が不正（同一対象へのロール矛盾、保持数の上限超過など）。
    #[error("Invalid privilege rule: {reason}")]
    InvalidPrivilege { reason: String },
    #[error("Invalid name: {reason}")]
    InvalidName { reason: String },
    #[error("Invalid Spatial ID: {reason}")]
    InvalidSpatialId { reason: String },
    #[error("Value type mismatch: expected {expected:?}, got {actual:?}")]
    ValueTypeMismatch {
        actual: JsonValueType,
        expected: JsonValueType,
    },
    #[error("Numeric value out of range: expected {expected}, got {actual}")]
    NumericValueOutOfRange { actual: String, expected: String },
    #[error("Constraint violation: {reason}")]
    ConstraintViolation { reason: String },
    #[error("Zoom level policy violation: expected max {max_zoom_level}, got {input_zoom_level}")]
    ZoomLevelPolicy {
        max_zoom_level: u8,
        input_zoom_level: u8,
    },
    #[error("Logic error: {0}")]
    LogicError(#[from] kasane_logic::Error),
    /// 同時実行または要求同士の食い違いで通せなかった。
    #[error("Conflict: {0}")]
    Conflict(String),

    // --- サーバー側の失敗（5xx）。原因で 3 つに分ける（モジュールの説明を参照） ---
    /// ディスク形式の版がこのビルドと合わない。`found` が `None` なら版を持たない世代。
    #[error("{}", match found { Some(found) => format!("On-disk schema version {found} is not supported by this build (it reads version {expected}); point the server at a fresh location"), None => format!("On-disk data predates schema versioning and is not supported by this build (it reads version {expected}); point the server at a fresh location") })]
    SchemaVersionMismatch { found: Option<u32>, expected: u32 },
    /// 保存されていたバイト列が読めない。
    #[error("Corrupt {stored}: {detail}")]
    Corrupt { stored: Stored, detail: String },
    /// ストレージエンジン自身が失敗した。
    ///
    /// feature で差し替えられるよう、具体的なエラー型は持ち込まずメッセージへ落とす。
    #[error("Storage error: {0}")]
    StorageError(String),
    /// このプログラムの不変条件が破れた（バグ）。
    #[error("Internal error: {0}")]
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

    fn grpc_code(&self) -> tonic::Code {
        match self {
            Self::Auth(e) => e.grpc_code(),
            Self::NotFound { .. } | Self::PrivilegeNotFound => tonic::Code::NotFound,
            Self::AlreadyExists { .. } => tonic::Code::AlreadyExists,
            // 同時実行の食い違いは、存在確認だけで判定できる `AlreadyExists` とは区別する。
            Self::Conflict(_) => tonic::Code::Aborted,
            Self::InvalidPrivilege { .. }
            | Self::InvalidName { .. }
            | Self::InvalidSpatialId { .. }
            | Self::ValueTypeMismatch { .. }
            | Self::NumericValueOutOfRange { .. }
            | Self::ConstraintViolation { .. }
            | Self::ZoomLevelPolicy { .. }
            | Self::LogicError(_) => tonic::Code::InvalidArgument,
            Self::SchemaVersionMismatch { .. }
            | Self::Corrupt { .. }
            | Self::StorageError(_)
            | Self::InternalError(_) => tonic::Code::Internal,
        }
    }
}

/// `code()` の機械可読コードは `google.rpc.ErrorInfo.reason` として details に載せる。
/// gRPC のクライアントは JSON 時代と同じ文字列で分岐できる。
const GRPC_ERROR_DOMAIN: &str = "kasane";

impl From<AppError> for tonic::Status {
    fn from(err: AppError) -> Self {
        use tonic_types::{ErrorDetails, StatusExt};

        let mut details = ErrorDetails::new();
        details.set_error_info(
            err.code(),
            GRPC_ERROR_DOMAIN,
            std::collections::HashMap::new(),
        );
        tonic::Status::with_error_details(err.grpc_code(), err.to_string(), details)
    }
}

impl From<AuthError> for tonic::Status {
    fn from(err: AuthError) -> Self {
        AppError::from(err).into()
    }
}
