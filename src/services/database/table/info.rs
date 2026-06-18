use crate::{AppState, error::AppError, models::database::table::TableInfoResponse};
use redb::ReadableDatabase;

pub async fn info(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
) -> Result<TableInfoResponse, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = crate::repositories::KasaneDbRead::new(read_txn);
    match db.table_info(db_name, table_name)? {
        Some(table) => {
            let count = db.table_count(table.id)?;
            Ok(TableInfoResponse {
                name: table.name,
                data_type: table.data_type,
                max_zoom_level: table.max_zoom_level,
                count,
            })
        }
        None => Err(AppError::TableNotFound {
            name: table_name.to_string(),
        }),
    }
}
