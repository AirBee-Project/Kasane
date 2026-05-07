use redb::ReadableDatabase;

use crate::{
    AppState, error::AppError, models::layer::Layer,
    repositories::layer::read::SpatialDbRead,
};

pub async fn list(app_state: &AppState) -> Result<Vec<Layer>, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = SpatialDbRead::new(read_txn);
    db.layer_list()
}
