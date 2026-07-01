use crate::{AppState, error::AppError};

pub async fn remove(app_state: &AppState, db_name: &str, table_name: &str) -> Result<(), AppError> {
    let app_state = app_state.clone();
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();

    tokio::task::spawn_blocking(move || {
        app_state
            .db
            .write(|db| db.table_remove(&db_name, &table_name))
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}
