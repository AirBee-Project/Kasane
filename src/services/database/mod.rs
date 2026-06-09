use crate::{AppState, error::AppError, models::database::DatabaseInfoResponse};
use redb::ReadableDatabase;

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

pub async fn list(app_state: &AppState) -> Result<Vec<DatabaseInfoResponse>, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = crate::repositories::KasaneDbRead::new(read_txn);
    db.database_list()
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
