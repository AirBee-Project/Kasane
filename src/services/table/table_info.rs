use redb::ReadableDatabase;

use crate::{
    AppState, error::AppError, models::table::Table, repositories::table::read::SpatialDbRead,
};

/// Services層からTableに関する情報を返す
pub async fn table_info(app_state: &AppState, table_name: &str) -> Result<Table, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = SpatialDbRead::new(read_txn);

    match db.table_info(table_name) {
        Ok(Some(table)) => Ok(table),
        Ok(None) => Err(AppError::TableNotFound {
            name: table_name.to_string(),
        }),
        Err(e) => Err(e),
    }
}
