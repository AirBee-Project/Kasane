use crate::{AppState, error::AppError, models::database::table::Table};

pub async fn copy(
    app_state: &AppState,
    src_db_name: &str,
    src_table_name: &str,
    copy_db_name: &str,
    copy_table_name: &str,
) -> Result<Table, AppError> {
    let app_state = app_state.clone();
    let src_db_name = src_db_name.to_string();
    let src_table_name = src_table_name.to_string();
    let copy_db_name = copy_db_name.to_string();
    let copy_table_name = copy_table_name.to_string();

    tokio::task::spawn_blocking(move || {
        app_state.db.write(|db| {
            db.table_copy(
                &src_db_name,
                &src_table_name,
                &copy_db_name,
                &copy_table_name,
            )
        })
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}
