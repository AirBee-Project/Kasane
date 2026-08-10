use crate::{
    AppState,
    error::{AppError, AuthError},
    models::users::{
        CreateUserRequest, PrivilegeRule, PrivilegeTarget, UpdatePasswordRequest, UserInfoResponse,
    },
    repositories::{CatalogRepository, ReadRepository, Storage, WriteRepository},
    services::auth::hash_password,
};
use uuid::Uuid;

pub async fn list_users(app_state: &AppState) -> Result<Vec<UserInfoResponse>, AppError> {
    app_state
        .db
        .read(async |r| {
            let mut out = Vec::new();
            for user in r.get_all_users().await? {
                out.push(UserInfoResponse {
                    privileges: r.render_privileges(&user.privileges).await?,
                    username: user.username,
                });
            }
            Ok(out)
        })
        .await
}

pub async fn get_user(app_state: &AppState, username: &str) -> Result<UserInfoResponse, AppError> {
    let username = username.to_string();
    app_state
        .db
        .read(async move |r| {
            let user = r.require_user(&username).await?;
            Ok(UserInfoResponse {
                privileges: r.render_privileges(&user.privileges).await?,
                username: user.username,
            })
        })
        .await
}

pub async fn get_privileges(
    app_state: &AppState,
    username: &str,
) -> Result<Vec<PrivilegeRule>, AppError> {
    let username = username.to_string();
    app_state
        .db
        .read(async move |r| {
            let user = r.require_user(&username).await?;
            r.render_privileges(&user.privileges).await
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
    if username == "root" {
        return Err(AuthError::RootProtected.into());
    }
    Ok(username.to_string())
}

pub async fn create_user(app_state: &AppState, req: CreateUserRequest) -> Result<(), AppError> {
    crate::services::helpers::name_valid::name_valid(&req.username)?;

    let hash = hash_password_off_thread(req.password.clone()).await?;
    let id = Uuid::now_v7();

    app_state
        .db
        .write(async move |w| {
            w.create_user(&req.username, id, hash, &req.privileges)
                .await
        })
        .await
}

pub async fn delete_user(app_state: &AppState, username: &str) -> Result<(), AppError> {
    let username = require_not_root(username)?;
    app_state
        .db
        .write(async move |w| w.delete_user(&username).await)
        .await
}

pub async fn update_password(
    app_state: &AppState,
    username: &str,
    req: UpdatePasswordRequest,
) -> Result<(), AppError> {
    let username = username.to_string();
    let hash = hash_password_off_thread(req.password).await?;

    app_state
        .db
        .write(async move |w| w.set_password(&username, hash).await)
        .await
}

/// 1 つの対象に対する権限を設定する（無ければ追加、あれば置き換え）。
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
