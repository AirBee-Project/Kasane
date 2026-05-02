use crate::{
    AppState,
    error::AppError,
    repositories::write::SpatialDbWrite,
    services::helpers::name_valid::name_valid,
};

pub async fn remove(app_state: &AppState, name: &str) -> Result<(), AppError> {
    let write_txn = app_state.redb.begin_write()?;
    let db = SpatialDbWrite::new(write_txn);

    let _ = name_valid(name)?;
    db.table_remove(name)?;
    db.commit()?;

    Ok(())
}