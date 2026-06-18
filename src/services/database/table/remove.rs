use crate::{AppState, error::AppError};

pub async fn remove(app_state: &AppState, db_name: &str, table_name: &str) -> Result<(), AppError> {
    let write_txn = app_state.db.env.write_txn()?;
    let mut db = crate::repositories::KasaneDbWrite::new(write_txn, &app_state.db);
    db.table_remove(db_name, table_name)?;
    db.commit()?;
    Ok(())
}
