use crate::{
    AppState,
    error::AppError,
    models::database::table::{TableListResponse, TableSummary},
};

pub async fn list(app_state: &AppState, db_name: &str) -> Result<TableListResponse, AppError> {
    let tables = app_state.db.read(|db| db.table_list(db_name))?;
    let response_tables = tables
        .into_iter()
        .map(|table| TableSummary {
            name: table.name,
            data_type: table.data_type,
            max_zoom_level: table.max_zoom_level,
            constraints: table.constraints,
        })
        .collect();
    Ok(TableListResponse(response_tables))
}
