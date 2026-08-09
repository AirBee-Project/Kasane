use crate::repositories::MetaRead;
use axum::{extract::Request, http::header, middleware::Next, response::Response};

use crate::models::id::DatabaseId;
use crate::models::users::{Scope, User, UserRole};
use crate::{
    AppState,
    error::{AppError, AuthError},
    services::auth::verify_jwt,
};

/// 認証済みユーザーをリクエスト拡張で運ぶための包み。
///
/// 権限判定そのものは [`User`] のメソッド（`can` / `has_global_role` など）にあり、
/// `Deref` 越しにそのまま使える。認可のドメインロジックが HTTP 層に漏れないようにするため。
#[derive(Clone)]
pub struct AuthUser {
    pub user: User,
}

impl std::ops::Deref for AuthUser {
    type Target = User;

    fn deref(&self) -> &Self::Target {
        &self.user
    }
}

pub async fn require_auth(
    axum::extract::State(app_state): axum::extract::State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = req.headers().get(header::AUTHORIZATION);

    let auth_header = match auth_header {
        Some(header) => header.to_str().unwrap_or(""),
        None => {
            return Err(AuthError::MissingToken.into());
        }
    };

    if !auth_header.starts_with("Bearer ") {
        return Err(AuthError::MalformedHeader.into());
    }

    let token = &auth_header[7..];
    let claims = verify_jwt(token)?;

    let user = app_state
        .db
        .read(|repo| repo.get_user(&claims.sub))?
        .ok_or(AppError::Auth(AuthError::TokenRevoked))?;

    if claims.uid != user.id.to_string() || claims.ver != user.token_version {
        return Err(AuthError::TokenRevoked.into());
    }

    req.extensions_mut().insert(AuthUser { user });

    Ok(next.run(req).await)
}

/// `Global` スコープで `required` 以上のロールを要求する。
/// データベースの作成・削除のように、特定のデータベースに紐づかない操作で使う。
pub fn check_global_role(user: &User, required: UserRole) -> Result<(), AppError> {
    if user.has_global_role(required) {
        Ok(())
    } else {
        Err(AuthError::RequiresGlobalAdmin.into())
    }
}

/// サーバー管理者（`global` スコープの `admin`）を要求する。
pub fn check_global_admin(user: &User) -> Result<(), AppError> {
    check_global_role(user, UserRole::Admin)
}

/// 本人、またはサーバー管理者であることを要求する。
pub fn check_self_or_admin(user: &User, username: &str) -> Result<(), AppError> {
    if user.is_global_admin() || user.username == username {
        Ok(())
    } else {
        Err(AuthError::NotSelfOrAdmin.into())
    }
}

/// データベース全体に対する操作の認可。テーブル単位のルールでは通らない。
pub fn check_database(
    app_state: &AppState,
    user: &User,
    db_name: &str,
    required: UserRole,
) -> Result<(), AppError> {
    check(app_state, user, db_name, None, required, Scope::Database)
}

/// データベースの存在確認・一覧のための認可。
/// テーブル単位のルールしか持たないユーザーも、自分のテーブルへ辿り着けるように通す。
pub fn check_database_visible(
    app_state: &AppState,
    user: &User,
    db_name: &str,
) -> Result<(), AppError> {
    check(app_state, user, db_name, None, UserRole::Read, Scope::AnyIn)
}

/// 特定テーブルに対する操作の認可。
pub fn check_table(
    app_state: &AppState,
    user: &User,
    db_name: &str,
    table_name: &str,
    required: UserRole,
) -> Result<(), AppError> {
    check(
        app_state,
        user,
        db_name,
        Some(table_name),
        required,
        Scope::Database,
    )
}

/// 複数テーブルに対する操作の認可。読み取りトランザクションを 1 つだけ開いて
/// すべての名前を解決するので、参照テーブル数に比例してトランザクションが増えない。
pub fn check_tables(
    app_state: &AppState,
    user: &User,
    tables: &[(&str, &str)],
    required: UserRole,
) -> Result<(), AppError> {
    if user.has_global_role(required) {
        return Ok(());
    }

    let scopes = app_state.db.read(|repo| {
        tables
            .iter()
            .map(|(db_name, table_name)| {
                resolve_scope(repo, db_name, Some(table_name), Scope::Database)
            })
            .collect::<Result<Vec<_>, _>>()
    })?;

    for (&(db_name, table_name), scope) in tables.iter().zip(scopes) {
        let allowed = scope.is_some_and(|scope| user.can(scope, required));
        if !allowed {
            return Err(denied(db_name, Some(table_name), required));
        }
    }
    Ok(())
}

fn denied(db_name: &str, table_name: Option<&str>, required: UserRole) -> AppError {
    AuthError::InsufficientPrivilege {
        db_name: db_name.to_string(),
        table_name: table_name.map(str::to_string),
        required,
    }
    .into()
}

/// 名前から判定対象のスコープを組み立てる。単一テーブル・複数テーブルどちらの検査でも
/// 同じ規則を使うため、ここに 1 箇所だけ置いている。
///
/// テーブル名が与えられていても解決できなかった場合は `db_scope` へフォールバックする。
/// これにより、そのデータベースに十分な権限を持つ利用者は認可を通過し、下位のサービス層が
/// 「テーブルが存在しない」として正しく 404 を返せる。権限のない利用者は
/// `db_scope` でも判定に落ちるので、存在有無を推測する手がかりにはならない。
///
/// データベース自体が解決できないときは `None`。呼び出し側は 404 ではなく 403 を返す
/// （権限のない利用者に名前の存在有無を教えないため）。
fn resolve_scope(
    repo: &crate::repositories::KasaneDbRead<'_>,
    db_name: &str,
    table_name: Option<&str>,
    db_scope: fn(DatabaseId) -> Scope,
) -> Result<Option<Scope>, AppError> {
    let Some(db_id) = repo.database_id(db_name)? else {
        return Ok(None);
    };
    let table_id = match table_name {
        Some(name) => repo.table_id(db_id, name)?,
        None => None,
    };
    Ok(Some(match table_id {
        Some(table_id) => Scope::Table(db_id, table_id),
        None => db_scope(db_id),
    }))
}

/// 名前 → ID を解決してスコープを組み立て、実効ロールを判定する。
/// `Global` ルールだけで足りる場合はカタログを引かずに決着させる。
fn check(
    app_state: &AppState,
    user: &User,
    db_name: &str,
    table_name: Option<&str>,
    required: UserRole,
    db_scope: fn(DatabaseId) -> Scope,
) -> Result<(), AppError> {
    if user.has_global_role(required) {
        return Ok(());
    }

    let scope = app_state
        .db
        .read(|repo| resolve_scope(repo, db_name, table_name, db_scope))?;

    if scope.is_some_and(|scope| user.can(scope, required)) {
        Ok(())
    } else {
        Err(denied(db_name, table_name, required))
    }
}
