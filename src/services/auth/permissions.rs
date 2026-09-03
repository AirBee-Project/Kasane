//! 認可ロジックと権限検査ヘルパー。
//!
//! **認可はカタログを引いたのと同じトランザクションの中で行う。** 認可のためだけに
//! 読み取りを開くと、直後にサービス層が同じ名前を引き直すことになり、名前の解決が
//! ネットワーク往復になるバックエンドではその往復が丸ごと二重になる。断面が分かれる
//! ぶん、判定した対象と操作した対象がずれる隙も生まれる。
//!
//! そのため、ここにある関数は `AppState` ではなく**開いているリポジトリ**を受け取り、
//! 解決した ID を呼び出し側へ返す。

use crate::error::{AppError, AuthError, Resource};
use crate::models::id::{DatabaseId, TableId};
use crate::models::users::{Scope, User, UserRole};
use crate::repositories::CatalogRepository;

/// 認証済みユーザーを運ぶための包み。
///
/// 権限判定は [`User`] 側にあり `Deref` 越しに使う。認可のドメインロジックを gRPC 層へ
/// 漏らさないため。JWT の検証とこの型の組み立ては [`crate::grpc::interceptor`] /
/// [`crate::grpc::auth_ctx`] が行う。
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

/// 特定のデータベースに紐づかない操作（データベースの作成・削除など）で使う。
///
/// ACL にもカタログにも触れない。全体ロールは利用者レコードに埋まっている。
#[tracing::instrument(skip_all)]
pub fn check_global_role(user: &User, required: UserRole) -> Result<(), AppError> {
    if user.has_global_role(required) {
        Ok(())
    } else {
        Err(AuthError::RequiresGlobalRole { required }.into())
    }
}

/// サーバー管理者（`global` スコープの `admin`）を要求する。
#[tracing::instrument(skip_all)]
pub fn check_global_admin(user: &User) -> Result<(), AppError> {
    check_global_role(user, UserRole::Admin)
}

/// 本人、またはサーバー管理者であることを要求する。
#[tracing::instrument(skip_all, fields(target_username = %username))]
pub fn check_self_or_admin(user: &User, username: &str) -> Result<(), AppError> {
    if user.is_global_admin() || user.username == username {
        Ok(())
    } else {
        Err(AuthError::NotSelfOrAdmin.into())
    }
}

/// 名前を解決し、その結果で認可する。
///
/// **「存在しない」を返すより先に呼ぶこと。** 逆にすると、権限の無い利用者へ 404 で
/// 名前の存在有無を教えることになる。データベースが解決できないときに 403 を返すのも
/// 同じ理由。
///
/// テーブル名が解決できなければデータベーススコープで判定する。そのデータベースに
/// 十分な権限を持つ利用者は通過して下位の層が 404 を返せるし、権限のない利用者は
/// そこでも落ちるので存在有無の手がかりにならない。
#[tracing::instrument(skip_all, fields(db_name = %db_name))]
pub async fn authorize_path<R: CatalogRepository>(
    repo: &R,
    user: &User,
    db_name: &str,
    table_name: Option<&str>,
    required: UserRole,
) -> Result<(), AppError> {
    // 全体ロールで足りるなら名前も ACL も引かない。
    if user.has_global_role(required) {
        return Ok(());
    }

    let db_id = repo.database_id(db_name).await?;
    let table_id = match (db_id, table_name) {
        (Some(db_id), Some(name)) => repo.table_id(db_id, name).await?,
        _ => None,
    };
    authorize_resolved(repo, user, db_id, table_id, db_name, table_name, required).await
}

/// **解決済みの** ID から認可する。名前は引き直さない。
///
/// 参照するテーブルをまとめて解決したあと（`resolve_tables`）に使う。
pub async fn authorize_resolved<R: CatalogRepository>(
    repo: &R,
    user: &User,
    db_id: Option<DatabaseId>,
    table_id: Option<TableId>,
    db_name: &str,
    table_name: Option<&str>,
    required: UserRole,
) -> Result<(), AppError> {
    // 解決できなければ ACL を引くまでもなく拒否（下位の層に 404 を出させない）。
    let scope = db_id.map(|db_id| match table_id {
        Some(table_id) => Scope::Table(db_id, table_id),
        None => Scope::Database(db_id),
    });

    match scope {
        Some(scope) if reaches(repo, user, scope, required).await? => Ok(()),
        _ => Err(denied(db_name, table_name, required)),
    }
}

/// このスコープに `required` 以上で届くか。
///
/// 判定そのものは [`User::allows`] にあり、ここは「全体ロールなら ACL を引かない」
/// という短絡だけを足す。**通す／拒む**を決める側（[`authorize_resolved`]）と
/// **見える／見えない**で絞る側（一覧）が同じ規則を共有するための入口。
pub async fn reaches<R: CatalogRepository>(
    repo: &R,
    user: &User,
    scope: Scope,
    required: UserRole,
) -> Result<bool, AppError> {
    if user.has_global_role(required) {
        return Ok(true);
    }
    Ok(user.allows(&repo.grant_for(user.id, scope).await?, required))
}

/// 「配下のどれかに届けば足りる」判定（存在確認・一覧）。**解決済みの値を受け取る。**
///
/// テーブル単位の権限しか持たない利用者も、自分のテーブルへ辿り着けるように通す。
/// 代わりに [`UserRole::Read`] より上は決して満たさない。
///
/// 権限が無ければ 403、権限はあるが存在しなければ 404。この順序が
/// 「権限の無い利用者へ名前の存在有無を教えない」を保つ。
///
/// 引いた結果をそのまま渡す形にしてあるので、呼び出し側は同じキーを 2 度読まない。
#[tracing::instrument(skip_all, fields(db_name = %db_name))]
pub async fn visible_database<R: CatalogRepository, T>(
    repo: &R,
    user: &User,
    db_name: &str,
    found: Option<(DatabaseId, T)>,
) -> Result<(DatabaseId, T), AppError> {
    let Some((db_id, value)) = found else {
        // 全体ロールを持つ利用者にだけ「無い」と教える。
        return Err(if user.has_global_role(UserRole::Read) {
            Resource::Database.not_found(db_name.to_string())
        } else {
            denied(db_name, None, UserRole::Read)
        });
    };

    if reaches(repo, user, Scope::AnyIn(db_id), UserRole::Read).await? {
        Ok((db_id, value))
    } else {
        Err(denied(db_name, None, UserRole::Read))
    }
}

fn denied(db_name: &str, table_name: Option<&str>, required: UserRole) -> AppError {
    AuthError::InsufficientPrivilege {
        db_name: db_name.to_string(),
        table_name: table_name.map(str::to_string),
        required,
    }
    .into()
}
