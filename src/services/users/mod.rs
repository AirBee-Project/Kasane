use crate::{
    AppState,
    error::{AppError, AuthError},
    models::users::{
        CreateUserRequest, PrivilegeRule, PrivilegeTarget, UpdatePasswordRequest, UserInfoResponse,
    },
    repositories::MetaRead,
    services::auth::hash_password,
};
use uuid::Uuid;

pub fn list_users(app_state: &AppState) -> Result<Vec<UserInfoResponse>, AppError> {
    app_state.db.read(|repo| {
        repo.get_all_users()?
            .into_iter()
            .map(|user| {
                Ok(UserInfoResponse {
                    privileges: repo.render_privileges(&user.privileges)?,
                    username: user.username,
                })
            })
            .collect()
    })
}

pub fn get_user(app_state: &AppState, username: &str) -> Result<UserInfoResponse, AppError> {
    app_state.db.read(|repo| {
        let user = repo.require_user(username)?;
        Ok(UserInfoResponse {
            privileges: repo.render_privileges(&user.privileges)?,
            username: user.username,
        })
    })
}

pub fn get_privileges(
    app_state: &AppState,
    username: &str,
) -> Result<Vec<PrivilegeRule>, AppError> {
    app_state.db.read(|repo| {
        let user = repo.require_user(username)?;
        repo.render_privileges(&user.privileges)
    })
}

pub async fn create_user(app_state: &AppState, req: CreateUserRequest) -> Result<(), AppError> {
    crate::services::helpers::name_valid::name_valid(&req.username)?;

    let app_state = app_state.clone();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let _guard = span.enter();
        let hash = hash_password(&req.password)?;
        let id = Uuid::now_v7();
        app_state
            .db
            .write(|repo| repo.create_user(&req.username, id, hash, &req.privileges))
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub async fn delete_user(app_state: &AppState, username: &str) -> Result<(), AppError> {
    if username == "root" {
        return Err(AuthError::RootProtected.into());
    }

    let state = app_state.clone();
    let username_owned = username.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        span.in_scope(|| state.db.write(|repo| repo.delete_user(&username_owned)))
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub async fn update_password(
    app_state: &AppState,
    username: &str,
    req: UpdatePasswordRequest,
) -> Result<(), AppError> {
    let state = app_state.clone();
    let username_owned = username.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let _guard = span.enter();
        let hash = hash_password(&req.password)?;
        state
            .db
            .write(|repo| repo.set_password(&username_owned, hash))
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

/// 1 つの対象に対する権限を設定する（無ければ追加、あれば置き換え）。
pub async fn grant_privilege(
    app_state: &AppState,
    username: &str,
    rule: PrivilegeRule,
) -> Result<(), AppError> {
    write_privilege(app_state, username, move |repo, username| {
        repo.grant_privilege(username, &rule)
    })
    .await
}

/// 1 つの対象に対する権限を剥奪する。
pub async fn revoke_privilege(
    app_state: &AppState,
    username: &str,
    target: PrivilegeTarget,
) -> Result<(), AppError> {
    write_privilege(app_state, username, move |repo, username| {
        repo.revoke_privilege(username, &target)
    })
    .await
}

/// 権限の書き込みに共通する下準備（root 保護とブロッキング実行）。
async fn write_privilege(
    app_state: &AppState,
    username: &str,
    apply: impl FnOnce(&mut crate::repositories::KasaneDbWrite<'_>, &str) -> Result<(), AppError>
    + Send
    + 'static,
) -> Result<(), AppError> {
    if username == "root" {
        return Err(AuthError::RootProtected.into());
    }

    let state = app_state.clone();
    let username = username.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let _guard = span.enter();
        state.db.write(|repo| apply(repo, &username))
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}
