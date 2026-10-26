use std::collections::HashMap;
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
    #[error("Privilege")]
    Privilege,
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

    const fn as_str(self) -> &'static str {
        match self {
            Self::Database => "database",
            Self::Table => "table",
            Self::User => "user",
            Self::Privilege => "privilege",
        }
    }
}

/// 読み出したバイト列が壊れていた対象。
///
/// その場で文言を書くと、同じ壊れ方が呼び出し元ごとに違うメッセージになる。
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
    /// トークンが無い・形式が不正・検証に失敗した・失効した。クライアントの対応はどれも再ログイン。
    #[error("Invalid authentication token: {0}")]
    InvalidToken(&'static str),
    /// ユーザーの存在有無を区別せず同一メッセージを返し、ユーザー列挙を防ぐ。
    #[error("Invalid username or password")]
    InvalidCredentials,
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    /// root ユーザーに対して許可されない操作（削除・権限変更など）。admin でも通らない。
    #[error("This operation is not allowed on the root user")]
    RootProtected,
}

impl AuthError {
    /// クライアントが分岐に使える安定した機械可読コード。
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidToken(_) => "invalid_token",
            Self::InvalidCredentials => "invalid_credentials",
            Self::PermissionDenied(_) => "permission_denied",
            Self::RootProtected => "root_protected",
        }
    }

    fn grpc_code(&self) -> tonic::Code {
        match self {
            Self::InvalidToken(_) | Self::InvalidCredentials => tonic::Code::Unauthenticated,
            Self::PermissionDenied(_) | Self::RootProtected => tonic::Code::PermissionDenied,
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    /// 認証・認可に関する失敗（[`AuthError`] を参照）。
    #[error("Authentication error: {0}")]
    Auth(#[from] AuthError),
    #[error("{resource} '{name}' not found")]
    NotFound { resource: Resource, name: String },
    #[error("{resource} '{name}' already exists")]
    AlreadyExists { resource: Resource, name: String },
    /// リクエストの内容が不正（名前、空間ID、ズームレベル、権限ルール、未設定のフィールドなど）。
    #[error("Invalid argument: {reason}")]
    InvalidArgument { reason: String },
    /// 値がテーブルの型・制約に合わない。
    #[error("Constraint violation: {reason}")]
    ConstraintViolation { reason: String },
    #[error("The request was cancelled")]
    Cancelled,
    /// サーバー側の失敗（ストレージの I/O、保存データの破損、ディスク形式の版の不一致、不変条件の破れ）。
    ///
    /// クライアントには区別する手段がないので 1 つにまとめ、原因はメッセージで伝える。
    #[error("Internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn invalid_argument(reason: impl Into<String>) -> Self {
        Self::InvalidArgument {
            reason: reason.into(),
        }
    }

    pub fn constraint_violation(reason: impl Into<String>) -> Self {
        Self::ConstraintViolation {
            reason: reason.into(),
        }
    }

    /// 保存されていたものが読めない。
    pub fn corrupt(stored: Stored, detail: impl fmt::Display) -> Self {
        Self::Internal(format!("Corrupt {stored}: {detail}"))
    }

    /// 保持できる権限数の上限に達した。付与側と解決側で同じ文言を使う。
    pub fn too_many_privileges() -> Self {
        Self::invalid_argument(format!(
            "a user cannot hold more than {} privileges",
            crate::models::users::MAX_PRIVILEGES_PER_USER
        ))
    }

    /// クライアントが分岐に使える安定した機械可読コード。
    fn code(&self) -> &'static str {
        match self {
            Self::Auth(e) => e.code(),
            Self::NotFound { .. } => "not_found",
            Self::AlreadyExists { .. } => "already_exists",
            Self::InvalidArgument { .. } => "invalid_argument",
            Self::ConstraintViolation { .. } => "constraint_violation",
            Self::Cancelled => "cancelled",
            Self::Internal(_) => "internal",
        }
    }

    fn grpc_code(&self) -> tonic::Code {
        match self {
            Self::Auth(e) => e.grpc_code(),
            Self::NotFound { .. } => tonic::Code::NotFound,
            Self::AlreadyExists { .. } => tonic::Code::AlreadyExists,
            Self::InvalidArgument { .. } | Self::ConstraintViolation { .. } => {
                tonic::Code::InvalidArgument
            }
            Self::Cancelled => tonic::Code::Cancelled,
            Self::Internal(_) => tonic::Code::Internal,
        }
    }

    /// `ErrorInfo.metadata` に載せる値。どのリソースの話かを、メッセージを解析せずに取れるようにする。
    fn metadata(&self) -> HashMap<String, String> {
        match self {
            Self::NotFound { resource, name } | Self::AlreadyExists { resource, name } => {
                HashMap::from([
                    ("resource".to_string(), resource.as_str().to_string()),
                    ("name".to_string(), name.clone()),
                ])
            }
            _ => HashMap::new(),
        }
    }
}

impl From<spatial_id::Error> for AppError {
    fn from(err: spatial_id::Error) -> Self {
        use spatial_id::query::QueryError;
        match err {
            spatial_id::Error::Query(QueryError::Cancelled) => Self::Cancelled,
            spatial_id::Error::Query(QueryError::SourceRead(reason)) => Self::Internal(reason),
            err => Self::invalid_argument(err.to_string()),
        }
    }
}

impl From<AppError> for tonic::Status {
    fn from(err: AppError) -> Self {
        use tonic_types::{ErrorDetails, StatusExt};

        let mut details = ErrorDetails::new();
        details.set_error_info(err.code(), "kasane", err.metadata());
        tonic::Status::with_error_details(err.grpc_code(), err.to_string(), details)
    }
}

impl From<AuthError> for tonic::Status {
    fn from(err: AuthError) -> Self {
        AppError::from(err).into()
    }
}
