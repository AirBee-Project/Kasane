use crate::{
    AppState,
    error::AppError,
    models::database::table::{TableListResponse, TableSummary},
};

pub async fn list(app_state: &AppState, db_name: &str) -> Result<TableListResponse, AppError> {
    let read_txn = app_state.db.env.read_txn()?;
    let db = crate::repositories::KasaneDbRead::new(read_txn, &app_state.db);
    let mut response_tables = Vec::new();
    for table in db.table_list(db_name)? {
        response_tables.push(TableSummary {
            name: table.name,
            data_type: table.data_type,
            max_zoom_level: table.max_zoom_level,
            constraints: table.constraints,
        });
    }
    Ok(TableListResponse(response_tables))
}
