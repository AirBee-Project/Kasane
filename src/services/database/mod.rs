use crate::{AppState, error::AppError, models::database::DatabaseInfoResponse};
use std::collections::HashSet;

pub async fn info(app_state: &AppState, name: &str) -> Result<DatabaseInfoResponse, AppError> {
    match app_state.db.read(|r| r.database_info(name))? {
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
    if is_global_admin {
        return app_state.db.read(|r| r.database_list());
    }

    // 一般ユーザーは権限を持つデータベースのみ閲覧可能
    let accessible: HashSet<String> = app_state
        .db
        .read_users(|repo| repo.get_user_privileges(user_id))?
        .into_iter()
        .map(|(db_name, _role)| db_name)
        .collect();

    let all = app_state.db.read(|r| r.database_list())?;
    Ok(all
        .into_iter()
        .filter(|info| accessible.contains(&info.name))
        .collect())
}

pub async fn create(app_state: &AppState, name: &str) -> Result<DatabaseInfoResponse, AppError> {
    crate::services::helpers::name_valid::name_valid(name)?;

    let app_state = app_state.clone();
    let name = name.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _guard = span.enter();
        app_state.db.write(|db| db.database_create(&name))
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub async fn remove(app_state: &AppState, name: &str) -> Result<(), AppError> {
    let app_state = app_state.clone();
    let name = name.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let _guard = span.enter();
        let tables = app_state.db.read(|r| r.table_list(&name))?;

        app_state.db.write(|db| {
            // First, list all tables and remove them
            for table in tables {
                db.table_remove(&name, &table.name)?;
            }
            db.database_remove(&name)?;
            Ok(())
        })
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub async fn rename(app_state: &AppState, name: &str, new_name: &str) -> Result<(), AppError> {
    let app_state = app_state.clone();
    let name = name.to_string();
    let new_name = new_name.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _guard = span.enter();
        app_state
            .db
            .write(|db| db.database_rename(&name, &new_name))
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub async fn copy(
    app_state: &AppState,
    name: &str,
    copy_name: &str,
    user_id: Option<crate::models::id::UserId>,
) -> Result<DatabaseInfoResponse, AppError> {
    let app_state = app_state.clone();
    let name = name.to_string();
    let copy_name = copy_name.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _guard = span.enter();
        app_state
            .db
            .write(|db| db.database_copy(&name, &copy_name, user_id))
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub mod table;
