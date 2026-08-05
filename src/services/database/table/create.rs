use crate::{
    AppState,
    error::AppError,
    models::database::table::{CreateTableRequest, Table},
};

pub async fn create(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    req: CreateTableRequest,
) -> Result<Table, AppError> {
    crate::services::helpers::name_valid::name_valid(table_name)?;

    let app_state = app_state.clone();
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        span.in_scope(|| {
            app_state
                .db
                .write(|db| db.table_create(&db_name, &table_name, req.data_type, req.constraints))
        })
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}
