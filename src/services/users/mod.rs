use crate::{
    AppState,
    error::{AppError, AuthError},
    models::id::PrincipalId,
    models::users::{
        CreateUserRequest, ListUsersQuery, PrivilegeRule, PrivilegeTarget, UpdatePasswordRequest,
        UserInfoResponse, UserListResponse, UserSummary,
    },
    repositories::{CatalogRepository, ReadRepository, Storage, WriteRepository},
    services::auth::hash_password,
};
use uuid::Uuid;

/// 1 ページぶんの利用者を返す。**権限そのものは含めない。**
///
/// 含めると 1 リクエストの読み取りが「利用者数 × 保持権限数」に比例する。概要だけなら
/// 利用者レコードを読むだけで済み、ページの大きさにしか比例しない。
#[tracing::instrument(skip_all)]
pub async fn list_users(
    app_state: &AppState,
    query: &ListUsersQuery,
) -> Result<UserListResponse, AppError> {
    let limit = query.page_size();
    let after = query.after.clone();

    // 続きがあるかを知るために 1 件多く引く。
    let mut page = app_state
        .db
        .read(async move |r| r.list_users(after.as_deref(), limit + 1).await)
        .await?;

    let next = (page.len() > limit)
        .then(|| page.pop())
        .flatten()
        .and(page.last().map(|(username, _)| username.clone()));

    Ok(UserListResponse {
        users: page
            .into_iter()
            .map(|(username, record)| UserSummary {
                username,
                global_role: record.global_role,
            })
            .collect(),
        next,
    })
}

#[tracing::instrument(skip_all, fields(username = %username))]
pub async fn get_user(app_state: &AppState, username: &str) -> Result<UserInfoResponse, AppError> {
    Ok(UserInfoResponse {
        privileges: get_privileges(app_state, username).await?,
        username: username.to_string(),
    })
}

#[tracing::instrument(skip_all, fields(username = %username))]
pub async fn get_privileges(
    app_state: &AppState,
    username: &str,
) -> Result<Vec<PrivilegeRule>, AppError> {
    let username = username.to_string();
    app_state
        .db
        .read(async move |r| {
            let record = r.require_user_record(&username).await?;
            let entries = r.acl_entries(record.id).await?;
            r.render_privileges(record.global_role, &entries).await
        })
        .await
}

/// パスワードをハッシュ化する。
///
/// argon2 は意図的に CPU を使うため、トランザクションの外側で専用のブロッキングタスクへ
/// 逃がす。書き込みクロージャの中で回すと、バックエンドによっては非同期ランタイムを
/// 塞いでしまう（クロージャはやり直しで複数回実行されうる点でも不利）。
async fn hash_password_off_thread(password: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?
}

/// root は削除・権限変更の対象にできない。
fn require_not_root(username: &str) -> Result<String, AppError> {
    if username == crate::repositories::ROOT_USERNAME {
        return Err(AuthError::RootProtected.into());
    }
    Ok(username.to_string())
}

#[tracing::instrument(skip_all, fields(username = %req.username))]
pub async fn create_user(app_state: &AppState, req: CreateUserRequest) -> Result<(), AppError> {
    crate::services::helpers::name_valid::name_valid(&req.username)?;

    let hash = hash_password_off_thread(req.password.clone()).await?;
    let id = PrincipalId(Uuid::now_v7());

    app_state
        .db
        .write(async move |w| {
            w.create_user(&req.username, id, hash.clone(), &req.privileges)
                .await
        })
        .await
}

#[tracing::instrument(skip_all, fields(username = %username))]
pub async fn delete_user(app_state: &AppState, username: &str) -> Result<(), AppError> {
    let username = require_not_root(username)?;
    app_state
        .db
        .write(async move |w| w.delete_user(&username).await)
        .await
}

#[tracing::instrument(skip_all, fields(username = %username))]
pub async fn update_password(
    app_state: &AppState,
    username: &str,
    req: UpdatePasswordRequest,
) -> Result<(), AppError> {
    let username = username.to_string();
    let hash = hash_password_off_thread(req.password).await?;

    app_state
        .db
        .write(async move |w| w.set_password(&username, hash.clone()).await)
        .await
}

/// 1 つの対象に対する権限を設定する（無ければ追加、あれば置き換え）。
#[tracing::instrument(skip_all, fields(username = %username))]
pub async fn grant_privilege(
    app_state: &AppState,
    username: &str,
    rule: PrivilegeRule,
) -> Result<(), AppError> {
    let username = require_not_root(username)?;
    app_state
        .db
        .write(async move |w| w.grant_privilege(&username, &rule).await)
        .await
}

/// 1 つの対象に対する権限を剥奪する。
#[tracing::instrument(skip_all, fields(username = %username))]
pub async fn revoke_privilege(
    app_state: &AppState,
    username: &str,
    target: PrivilegeTarget,
) -> Result<(), AppError> {
    let username = require_not_root(username)?;
    app_state
        .db
        .write(async move |w| w.revoke_privilege(&username, &target).await)
        .await
}
