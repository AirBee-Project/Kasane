use crate::{
    AppState, error::AppError, models::database::DatabaseInfoResponse,
    repositories::users::KasaneUsersRead,
};
use std::collections::HashSet;

pub async fn info(app_state: &AppState, name: &str) -> Result<DatabaseInfoResponse, AppError> {
    let read_txn = app_state.db.env.read_txn()?;
    let db = crate::repositories::KasaneDbRead::new(read_txn, &app_state.db);
    match db.database_info(name)? {
        Some(info) => Ok(info),
        None => Err(AppError::DatabaseNotFound {
            name: name.to_string(),
        }),
    }
}

/// データベース一覧を取得する。
///
/// GlobalAdmin は全データベースを取得できる。それ以外のユーザーは、
/// 何らかの権限（Read 以上）を保持しているデータベースのみを取得できる。
pub async fn list(
    app_state: &AppState,
    is_global_admin: bool,
    user_id: crate::models::id::UserId,
) -> Result<Vec<DatabaseInfoResponse>, AppError> {
    let read_txn = app_state.db.env.read_txn()?;

    if is_global_admin {
        let db = crate::repositories::KasaneDbRead::new(read_txn, &app_state.db);
        return db.database_list();
    }

    // 一般ユーザーは権限を持つデータベースのみ閲覧可能
    let users_repo = KasaneUsersRead::new(read_txn, &app_state.db);
    let accessible: HashSet<String> = users_repo
        .get_user_privileges(user_id)?
        .into_iter()
        .map(|(db_name, _role)| db_name)
        .collect();

    drop(users_repo);

    let read_txn = app_state.db.env.read_txn()?;
    let db = crate::repositories::KasaneDbRead::new(read_txn, &app_state.db);
    let all = db.database_list()?;
    Ok(all
        .into_iter()
        .filter(|info| accessible.contains(&info.name))
        .collect())
}

pub async fn create(app_state: &AppState, name: &str) -> Result<DatabaseInfoResponse, AppError> {
    crate::services::helpers::name_valid::name_valid(name)?;

    let app_state = app_state.clone();
    let name = name.to_string();

    tokio::task::spawn_blocking(move || -> Result<DatabaseInfoResponse, AppError> {
        let write_txn = app_state.db.env.write_txn()?;
        let mut db = crate::repositories::KasaneDbWrite::new(write_txn, &app_state.db);
        let res = db.database_create(&name)?;
        db.commit()?;
        Ok(res)
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub async fn remove(app_state: &AppState, name: &str) -> Result<(), AppError> {
    let app_state = app_state.clone();
    let name = name.to_string();

    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let tables = {
            let read_txn = app_state.db.env.read_txn()?;
            let db = crate::repositories::KasaneDbRead::new(read_txn, &app_state.db);
            db.table_list(&name)?
        };

        let write_txn = app_state.db.env.write_txn()?;
        let mut db = crate::repositories::KasaneDbWrite::new(write_txn, &app_state.db);

        // First, list all tables and remove them
        for table in tables {
            db.table_remove(&name, &table.name)?;
        }

        db.database_remove(&name)?;
        db.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub async fn rename(app_state: &AppState, name: &str, new_name: &str) -> Result<(), AppError> {
    let app_state = app_state.clone();
    let name = name.to_string();
    let new_name = new_name.to_string();

    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let write_txn = app_state.db.env.write_txn()?;
        let mut db = crate::repositories::KasaneDbWrite::new(write_txn, &app_state.db);
        db.database_rename(&name, &new_name)?;
        db.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub async fn copy(
    app_state: &AppState,
    name: &str,
    destination_name: &str,
    user_id: Option<crate::models::id::UserId>,
) -> Result<DatabaseInfoResponse, AppError> {
    let app_state = app_state.clone();
    let name = name.to_string();
    let destination_name = destination_name.to_string();

    tokio::task::spawn_blocking(move || -> Result<DatabaseInfoResponse, AppError> {
        let write_txn = app_state.db.env.write_txn()?;
        let mut db = crate::repositories::KasaneDbWrite::new(write_txn, &app_state.db);
        let res = db.database_copy(&name, &destination_name, user_id)?;
        db.commit()?;
        Ok(res)
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub mod table;
