use crate::{
    AppState,
    error::AppError,
    models::database::table::{TableInfoResponse, TableListResponse},
};
use redb::ReadableDatabase;

pub async fn list(app_state: &AppState, db_name: &str) -> Result<TableListResponse, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = crate::repositories::KasaneDbRead::new(read_txn);
    let tables = db.table_list(db_name)?;
    Ok(TableListResponse(
        tables.into_iter().map(TableInfoResponse::from).collect(),
    ))
}
