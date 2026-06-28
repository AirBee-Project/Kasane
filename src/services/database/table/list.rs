use crate::{
    AppState,
    error::AppError,
    models::database::table::{TableInfoResponse, TableListResponse},
};

pub async fn list(app_state: &AppState, db_name: &str) -> Result<TableListResponse, AppError> {
    let read_txn = app_state.db.env.read_txn()?;
    let db = crate::repositories::KasaneDbRead::new(read_txn, &app_state.db);
    let mut response_tables = Vec::new();
    for table in db.table_list(db_name)? {
        response_tables.push(TableInfoResponse {
            name: table.name,
            data_type: table.data_type,
            max_zoom_level: table.max_zoom_level,
            count: None,
        });
    }
    Ok(TableListResponse(response_tables))
}
