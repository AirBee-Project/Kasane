use redb::ReadableDatabase;

use crate::{
    AppState, error::AppError, models::table::Table, repositories::read::SpatialDbRead,
};

pub async fn table_list(app_state: &AppState) -> Result<Vec<Table>, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = SpatialDbRead::new(read_txn);
    db.table_list()
}
