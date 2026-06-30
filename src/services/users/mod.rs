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
use uuid::Uuid;

pub fn list_users(app_state: &AppState) -> Result<Vec<UserInfoResponse>, AppError> {
    let read_txn = app_state.db.env.read_txn()?;
    let repo = KasaneUsersRead::new(read_txn, &app_state.db);
    let users = repo.get_all_users()?;
    Ok(users.into_iter().map(UserInfoResponse::from).collect())
}

pub fn get_user(app_state: &AppState, username: &str) -> Result<UserInfoResponse, AppError> {
    let read_txn = app_state.db.env.read_txn()?;
    let repo = KasaneUsersRead::new(read_txn, &app_state.db);
    let user = repo
        .get_user(username)?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    Ok(UserInfoResponse::from(user))
}

pub async fn create_user(app_state: &AppState, req: CreateUserRequest) -> Result<(), AppError> {
    crate::services::helpers::name_valid::name_valid(&req.username)?;

    let app_state = app_state.clone();

    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let hash = hash_password(&req.password)?;
        let meta = UserMetadata {
            id: Uuid::now_v7(),
            password_hash: hash,
            is_global_admin: req.is_global_admin,
            token_version: 0,
        };
        let write_txn = app_state.db.env.write_txn()?;
        let mut repo = KasaneUsersWrite::new(write_txn, &app_state.db);
        repo.create_user(&req.username, &meta)?;
        repo.commit()?;
        Ok(())
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

    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let write_txn = state.db.env.write_txn()?;
        let mut repo = KasaneUsersWrite::new(write_txn, &state.db);
        repo.delete_user(&username_owned)?;
        repo.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))??;

    app_state.auth_cache.remove(username);
    Ok(())
}

pub async fn update_password(
    app_state: &AppState,
    username: &str,
    req: UpdatePasswordRequest,
) -> Result<(), AppError> {
    let state = app_state.clone();
    let username_owned = username.to_string();

    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let hash = hash_password(&req.password)?;

        let meta = {
            let read_txn = state.db.env.read_txn()?;
            let repo = KasaneUsersRead::new(read_txn, &state.db);
            repo.get_user_meta(&username_owned)?
                .ok_or_else(|| AppError::NotFound("User not found".into()))?
        };

        let mut new_meta = meta;
        new_meta.password_hash = hash;
        new_meta.token_version = new_meta.token_version.wrapping_add(1);

        let write_txn = state.db.env.write_txn()?;
        let mut repo = KasaneUsersWrite::new(write_txn, &state.db);
        repo.update_user_meta(&username_owned, &new_meta)?;
        repo.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))??;

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

    let state = app_state.clone();
    let username_owned = username.to_string();

    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let meta = {
            let read_txn = state.db.env.read_txn()?;
            let repo = KasaneUsersRead::new(read_txn, &state.db);
            repo.get_user_meta(&username_owned)?
                .ok_or_else(|| AppError::NotFound("User not found".into()))?
        };

        let mut new_meta = meta;
        new_meta.is_global_admin = is_global_admin;
        new_meta.token_version = new_meta.token_version.wrapping_add(1);

        let write_txn = state.db.env.write_txn()?;
        let mut repo = KasaneUsersWrite::new(write_txn, &state.db);
        repo.update_user_meta(&username_owned, &new_meta)?;
        repo.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))??;

    app_state.auth_cache.remove(username);
    Ok(())
}

pub async fn set_privilege(
    app_state: &AppState,
    username: &str,
    db_name: &str,
    req: UpdatePrivilegeRequest,
) -> Result<(), AppError> {
    let app_state = app_state.clone();
    let username = username.to_string();
    let db_name = db_name.to_string();

    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let user_id = {
            let read_txn = app_state.db.env.read_txn()?;
            let repo = KasaneUsersRead::new(read_txn, &app_state.db);
            let user = repo
                .get_user(&username)?
                .ok_or_else(|| AppError::NotFound("User not found".into()))?;
            user.id
        };
        let write_txn = app_state.db.env.write_txn()?;
        let mut repo = KasaneUsersWrite::new(write_txn, &app_state.db);
        repo.set_privilege(crate::models::id::UserId(user_id), &db_name, req.role)?;
        repo.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub async fn delete_privilege(
    app_state: &AppState,
    username: &str,
    db_name: &str,
) -> Result<(), AppError> {
    let app_state = app_state.clone();
    let username = username.to_string();
    let db_name = db_name.to_string();

    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let user_id = {
            let read_txn = app_state.db.env.read_txn()?;
            let repo = KasaneUsersRead::new(read_txn, &app_state.db);
            let user = repo
                .get_user(&username)?
                .ok_or_else(|| AppError::NotFound("User not found".into()))?;
            user.id
        };
        let write_txn = app_state.db.env.write_txn()?;
        let mut repo = KasaneUsersWrite::new(write_txn, &app_state.db);
        repo.remove_privilege(crate::models::id::UserId(user_id), &db_name)?;
        repo.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub fn get_privileges(
    app_state: &AppState,
    username: &str,
) -> Result<Vec<PrivilegeInfoResponse>, AppError> {
    let read_txn = app_state.db.env.read_txn()?;
    let repo = KasaneUsersRead::new(read_txn, &app_state.db);
    let user = repo
        .get_user(username)?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    let privs = repo.get_user_privileges(crate::models::id::UserId(user.id))?;
    Ok(privs
        .into_iter()
        .map(|(db_name, role)| PrivilegeInfoResponse { db_name, role })
        .collect())
}
