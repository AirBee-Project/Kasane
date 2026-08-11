use crate::{
    AppState,
    error::AppError,
    models::database::table::{CreateTableRequest, Table},
    repositories::{Storage, WriteRepository},
};

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn create(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    req: CreateTableRequest,
) -> Result<Table, AppError> {
    crate::services::helpers::name_valid::name_valid(table_name)?;
    if let Some(desc) = &req.description
        && desc.chars().count() > crate::models::database::MAX_DESCRIPTION_LENGTH
    {
        return Err(AppError::InvalidName {
            reason: format!(
                "Description cannot exceed {} characters",
                crate::models::database::MAX_DESCRIPTION_LENGTH
            ),
        });
    }

    // max_zoom_levelの検証
    kasane_logic::ZoomLevel::new(req.max_zoom_level)?;

    let db_name = db_name.to_string();
    let table_name = table_name.to_string();

    app_state
        .db
        .write(async move |db| {
            db.table_create(
                &db_name,
                &table_name,
                req.data_type,
                req.max_zoom_level,
                req.constraints,
                req.description,
                req.value_index,
            )
            .await
        })
        .await
}
