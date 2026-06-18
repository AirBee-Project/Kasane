use crate::{
    AppState,
    error::AppError,
    models::database::table::{TableInfoResponse, TableListResponse},
};
use redb::ReadableDatabase;

pub async fn list(app_state: &AppState, db_name: &str) -> Result<TableListResponse, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = crate::repositories::KasaneDbRead::new(read_txn);
    let mut response_tables = Vec::new();
    for table in db.table_list(db_name)? {
        let count = db.table_count(table.id)?;
        response_tables.push(TableInfoResponse {
            name: table.name,
            data_type: table.data_type,
            max_zoom_level: table.max_zoom_level,
            count,
        });
    }
    Ok(TableListResponse(response_tables))
}
