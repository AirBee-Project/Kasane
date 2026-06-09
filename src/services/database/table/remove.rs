use crate::{AppState, error::AppError};

pub async fn remove(app_state: &AppState, db_name: &str, table_name: &str) -> Result<(), AppError> {
    let write_txn = app_state.redb.begin_write()?;
    let mut db = crate::repositories::KasaneDbWrite::new(write_txn);
    db.table_remove(db_name, table_name)?;
    db.commit()?;
    Ok(())
}
