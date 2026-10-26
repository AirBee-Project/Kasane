use crate::{
    Kasane,
    error::{AppError, AuthError},
    models::id::PrincipalId,
    models::users::{
        CreateUserRequest, ListUsersQuery, PrivilegeRule, PrivilegeTarget, UpdatePasswordRequest,
        User, UserDetail, UserPage, UserSummary,
    },
    repositories::{CatalogRepository, ReadRepository, Storage, WriteRepository},
    services::auth::{check_global_admin, check_self_or_admin, hash_password},
};
use uuid::Uuid;

/// 1 ページぶんの利用者を返す。**権限そのものは含めない。**
///
/// 含めると 1 リクエストの読み取りが「利用者数 × 保持権限数」に比例する。概要だけなら
/// 利用者レコードを読むだけで済み、ページの大きさにしか比例しない。
#[tracing::instrument(skip_all)]
pub async fn list_users(
    kasane: &Kasane,
    user: &User,
    query: &ListUsersQuery,
) -> Result<UserPage, AppError> {
    check_global_admin(user)?;
    let limit = query.page_size();
    let after = query.after.clone();

    // 続きがあるかを知るために 1 件多く引く。
    let mut page = kasane
        .db
        .read(async move |r| r.list_users(after.as_deref(), limit + 1).await)
        .await?;

    let next = (page.len() > limit)
        .then(|| page.pop())
        .flatten()
        .and(page.last().map(|(username, _)| username.clone()));

    Ok(UserPage {
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
pub async fn get_user(
    kasane: &Kasane,
    user: &User,
    username: &str,
) -> Result<UserDetail, AppError> {
    check_self_or_admin(user, username)?;
    let owned = username.to_string();
    let privileges = kasane
        .db
        .read(async move |r| {
            let record = r.require_user_record(&owned).await?;
            let entries = r.acl_entries(record.id).await?;
            r.render_privileges(record.global_role, &entries).await
        })
        .await?;
    Ok(UserDetail {
        privileges,
        username: username.to_string(),
    })
}

/// パスワードをハッシュ化する。
///
/// argon2 は意図的に CPU を使うため、トランザクションの外側で専用のブロッキングタスクへ
/// 逃がす。書き込みクロージャの中で回すと、バックエンドによっては非同期ランタイムを
/// 塞いでしまう（クロージャはやり直しで複数回実行されうる点でも不利）。
async fn hash_password_off_thread(password: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

/// root は削除・権限変更の対象にできない。
fn require_not_root(username: &str) -> Result<(), AppError> {
    if username == crate::repositories::ROOT_USERNAME {
        return Err(AuthError::RootProtected.into());
    }
    Ok(())
}

#[tracing::instrument(skip_all, fields(username = %req.username))]
pub async fn create_user(
    kasane: &Kasane,
    user: &User,
    req: CreateUserRequest,
) -> Result<(), AppError> {
    check_global_admin(user)?;
    crate::services::helpers::name_valid::name_valid(&req.username)?;

    let hash = hash_password_off_thread(req.password.clone()).await?;
    let id = PrincipalId(Uuid::now_v7());

    kasane
        .db
        .write(async move |w| {
            w.create_user(&req.username, id, hash.clone(), &req.privileges)
                .await
        })
        .await
}

#[tracing::instrument(skip_all, fields(username = %username))]
pub async fn delete_user(kasane: &Kasane, user: &User, username: &str) -> Result<(), AppError> {
    check_global_admin(user)?;
    require_not_root(username)?;
    let username = username.to_string();
    kasane
        .db
        .write(async move |w| w.delete_user(&username).await)
        .await
}

#[tracing::instrument(skip_all, fields(username = %username))]
pub async fn update_password(
    kasane: &Kasane,
    user: &User,
    username: &str,
    req: UpdatePasswordRequest,
) -> Result<(), AppError> {
    check_self_or_admin(user, username)?;
    let username = username.to_string();
    let hash = hash_password_off_thread(req.password).await?;

    kasane
        .db
        .write(async move |w| w.set_password(&username, hash.clone()).await)
        .await
}

/// 1 つの対象に対する権限を設定する（無ければ追加、あれば置き換え）。
#[tracing::instrument(skip_all, fields(username = %username))]
pub async fn grant_privilege(
    kasane: &Kasane,
    user: &User,
    username: &str,
    rule: PrivilegeRule,
) -> Result<(), AppError> {
    check_global_admin(user)?;
    require_not_root(username)?;
    let username = username.to_string();
    kasane
        .db
        .write(async move |w| w.grant_privilege(&username, &rule).await)
        .await
}

/// 1 つの対象に対する権限を剥奪する。
#[tracing::instrument(skip_all, fields(username = %username))]
pub async fn revoke_privilege(
    kasane: &Kasane,
    user: &User,
    username: &str,
    target: PrivilegeTarget,
) -> Result<(), AppError> {
    check_global_admin(user)?;
    require_not_root(username)?;
    let username = username.to_string();
    kasane
        .db
        .write(async move |w| w.revoke_privilege(&username, &target).await)
        .await
}
