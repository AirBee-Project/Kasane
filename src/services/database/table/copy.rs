use crate::{AppState, error::AppError, models::database::table::Table};

pub async fn copy(
    app_state: &AppState,
    src_db_name: &str,
    src_table_name: &str,
    dest_db_name: &str,
    dest_table_name: &str,
) -> Result<Table, AppError> {
    let app_state = app_state.clone();
    let src_db_name = src_db_name.to_string();
    let src_table_name = src_table_name.to_string();
    let dest_db_name = dest_db_name.to_string();
    let dest_table_name = dest_table_name.to_string();

    tokio::task::spawn_blocking(move || {
        app_state.db.write(|db| {
            db.table_copy(
                &src_db_name,
                &src_table_name,
                &dest_db_name,
                &dest_table_name,
            )
        })
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}
