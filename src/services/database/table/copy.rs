use crate::{
    AppState,
    error::AppError,
    models::database::table::Table,
    repositories::{Storage, WriteRepository},
};

pub async fn copy(
    app_state: &AppState,
    src_db_name: &str,
    src_table_name: &str,
    copy_db_name: &str,
    copy_table_name: &str,
) -> Result<Table, AppError> {
    let src_db_name = src_db_name.to_string();
    let src_table_name = src_table_name.to_string();
    let copy_db_name = copy_db_name.to_string();
    let copy_table_name = copy_table_name.to_string();

    app_state
        .db
        .write(async move |db| {
            db.table_copy(
                &src_db_name,
                &src_table_name,
                &copy_db_name,
                &copy_table_name,
            )
            .await
        })
        .await
}
