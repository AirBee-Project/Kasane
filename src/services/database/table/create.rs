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

    // max_zoom_levelの検証
    kasane_logic::ZoomLevel::new(req.max_zoom_level)?;

    let app_state = app_state.clone();
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        span.in_scope(|| {
            app_state.db.write(|db| {
                db.table_create(
                    &db_name,
                    &table_name,
                    req.data_type,
                    req.max_zoom_level,
                    req.constraints,
                )
            })
        })
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}
