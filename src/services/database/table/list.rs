use crate::{
    AppState,
    error::AppError,
    models::database::table::{TableListResponse, TableSummary},
};

#[tracing::instrument(skip_all, fields(db_name = %db_name))]
pub async fn list(app_state: &AppState, db_name: &str) -> Result<TableListResponse, AppError> {
    let tables = app_state.db.read(|db| db.table_list(db_name))?;
    let response_tables = tables
        .into_iter()
        .map(|table| TableSummary {
            name: table.name,
            data_type: table.data_type,
            constraints: table.constraints,
        })
        .collect();
    Ok(TableListResponse(response_tables))
}
