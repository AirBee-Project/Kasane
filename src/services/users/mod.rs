use crate::{
    AppState,
    error::{AppError, AuthError},
    models::users::{
        CreateUserRequest, PrivilegeInfoResponse, UpdatePasswordRequest, UpdatePrivilegeRequest,
        UserInfoResponse, UserMetadata,
    },
    repositories::users::{KasaneUsersRead, KasaneUsersWrite},
    services::auth::hash_password,
};
use redb::ReadableDatabase;
use uuid::Uuid;

pub fn list_users(app_state: &AppState) -> Result<Vec<UserInfoResponse>, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let repo = KasaneUsersRead::new(read_txn);
    let users = repo.get_all_users()?;
    Ok(users.into_iter().map(UserInfoResponse::from).collect())
}

pub async fn create_user(app_state: &AppState, req: CreateUserRequest) -> Result<(), AppError> {
    crate::services::helpers::name_valid::name_valid(&req.username)?;
    let hash = hash_password(&req.password)?;
    let meta = UserMetadata {
        id: Uuid::now_v7(),
        password_hash: hash,
        is_global_admin: req.is_global_admin,
        token_version: 0,
    };
    let write_txn = app_state.redb.begin_write()?;
    let mut repo = KasaneUsersWrite::new(write_txn);
    repo.create_user(&req.username, &meta)?;
    repo.commit()?;
    Ok(())
}

pub async fn delete_user(app_state: &AppState, username: &str) -> Result<(), AppError> {
    if username == "root" {
        return Err(AuthError::RootProtected.into());
    }

    let write_txn = app_state.redb.begin_write()?;
    let mut repo = KasaneUsersWrite::new(write_txn);
    repo.delete_user(username)?;
    repo.commit()?;

    app_state.auth_cache.remove(username);
    Ok(())
}

pub async fn update_password(
    app_state: &AppState,
    username: &str,
    req: UpdatePasswordRequest,
) -> Result<(), AppError> {
    let hash = hash_password(&req.password)?;

    let meta = {
        let read_txn = app_state.redb.begin_read()?;
        let repo = KasaneUsersRead::new(read_txn);
        repo.get_user_meta(username)?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?
    };

    let mut new_meta = meta;
    new_meta.password_hash = hash;
    new_meta.token_version = new_meta.token_version.wrapping_add(1);

    let write_txn = app_state.redb.begin_write()?;
    let mut repo = KasaneUsersWrite::new(write_txn);
    repo.update_user_meta(username, &new_meta)?;
    repo.commit()?;

    app_state.auth_cache.remove(username);
    Ok(())
}

/// ユーザーの GlobalAdmin 権限を付与・剥奪する。
pub async fn set_admin(
    app_state: &AppState,
    username: &str,
    is_global_admin: bool,
) -> Result<(), AppError> {
    if username == "root" {
        return Err(AuthError::RootProtected.into());
    }

    let meta = {
        let read_txn = app_state.redb.begin_read()?;
        let repo = KasaneUsersRead::new(read_txn);
        repo.get_user_meta(username)?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?
    };

    let mut new_meta = meta;
    new_meta.is_global_admin = is_global_admin;
    new_meta.token_version = new_meta.token_version.wrapping_add(1);

    let write_txn = app_state.redb.begin_write()?;
    let mut repo = KasaneUsersWrite::new(write_txn);
    repo.update_user_meta(username, &new_meta)?;
    repo.commit()?;

    app_state.auth_cache.remove(username);
    Ok(())
}

pub async fn set_privilege(
    app_state: &AppState,
    username: &str,
    db_name: &str,
    req: UpdatePrivilegeRequest,
) -> Result<(), AppError> {
    let user_id = {
        let read_txn = app_state.redb.begin_read()?;
        let repo = KasaneUsersRead::new(read_txn);
        let user = repo
            .get_user(username)?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;
        user.id.into_bytes()
    };
    let write_txn = app_state.redb.begin_write()?;
    let mut repo = KasaneUsersWrite::new(write_txn);
    repo.set_privilege(user_id, db_name, req.role)?;
    repo.commit()?;
    Ok(())
}

pub async fn delete_privilege(
    app_state: &AppState,
    username: &str,
    db_name: &str,
) -> Result<(), AppError> {
    let user_id = {
        let read_txn = app_state.redb.begin_read()?;
        let repo = KasaneUsersRead::new(read_txn);
        let user = repo
            .get_user(username)?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;
        user.id.into_bytes()
    };
    let write_txn = app_state.redb.begin_write()?;
    let mut repo = KasaneUsersWrite::new(write_txn);
    repo.remove_privilege(user_id, db_name)?;
    repo.commit()?;
    Ok(())
}

pub fn get_privileges(
    app_state: &AppState,
    username: &str,
) -> Result<Vec<PrivilegeInfoResponse>, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let repo = KasaneUsersRead::new(read_txn);
    let user = repo
        .get_user(username)?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    let privs = repo.get_user_privileges(user.id.into_bytes())?;
    Ok(privs
        .into_iter()
        .map(|(db_name, role)| PrivilegeInfoResponse { db_name, role })
        .collect())
}
