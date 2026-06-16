use crate::{
    AppState, error::AppError, models::database::DatabaseInfoResponse,
    repositories::users::KasaneUsersRead,
};
use redb::ReadableDatabase;
use std::collections::HashSet;

pub async fn info(app_state: &AppState, name: &str) -> Result<DatabaseInfoResponse, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = crate::repositories::KasaneDbRead::new(read_txn);
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
    user_id: [u8; 16],
) -> Result<Vec<DatabaseInfoResponse>, AppError> {
    let read_txn = app_state.redb.begin_read()?;

    if is_global_admin {
        let db = crate::repositories::KasaneDbRead::new(read_txn);
        return db.database_list();
    }

    // 一般ユーザーは権限を持つデータベースのみ閲覧可能
    let users_repo = KasaneUsersRead::new(read_txn);
    let accessible: HashSet<String> = users_repo
        .get_user_privileges(user_id)?
        .into_iter()
        .map(|(db_name, _role)| db_name)
        .collect();

    let read_txn = app_state.redb.begin_read()?;
    let db = crate::repositories::KasaneDbRead::new(read_txn);
    let all = db.database_list()?;
    Ok(all
        .into_iter()
        .filter(|info| accessible.contains(&info.name))
        .collect())
}

pub async fn create(app_state: &AppState, name: &str) -> Result<DatabaseInfoResponse, AppError> {
    crate::services::helpers::name_valid::name_valid(name)?;
    let write_txn = app_state.redb.begin_write()?;
    let mut db = crate::repositories::KasaneDbWrite::new(write_txn);
    let res = db.database_create(name)?;
    db.commit()?;
    Ok(res)
}

pub async fn remove(app_state: &AppState, name: &str) -> Result<(), AppError> {
    let tables = {
        let read_txn = app_state.redb.begin_read()?;
        let db = crate::repositories::KasaneDbRead::new(read_txn);
        db.table_list(name)?
    };

    let write_txn = app_state.redb.begin_write()?;
    let mut db = crate::repositories::KasaneDbWrite::new(write_txn);

    // First, list all tables and remove them
    for table in tables {
        db.table_remove(name, &table.name)?;
    }

    db.database_remove(name)?;
    db.commit()?;
    Ok(())
}

pub mod table;
