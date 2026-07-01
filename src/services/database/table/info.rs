use crate::{AppState, error::AppError, models::database::table::TableInfoResponse};

pub async fn info(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
) -> Result<TableInfoResponse, AppError> {
    app_state
        .db
        .read(|db| match db.table_info(db_name, table_name)? {
            Some(table) => {
                let count = db.table_count(table.id)?;
                Ok(TableInfoResponse {
                    name: table.name,
                    data_type: table.data_type,
                    max_zoom_level: table.max_zoom_level,
                    count,
                    constraints: table.constraints,
                })
            }
            None => Err(AppError::TableNotFound {
                name: table_name.to_string(),
            }),
        })
}
