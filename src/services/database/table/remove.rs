use crate::{
    AppState,
    error::AppError,
    repositories::{Storage, WriteRepository},
};

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn remove(app_state: &AppState, db_name: &str, table_name: &str) -> Result<(), AppError> {
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();

    app_state
        .db
        .write(async move |db| db.table_remove(&db_name, &table_name).await)
        .await
}
