use crate::{
    AppState,
    error::AppError,
    models::database::table::TableSummary,
    repositories::{Storage, WriteRepository},
};

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn table_update(
    state: AppState,
    db_name: &str,
    table_name: &str,
    new_name: Option<&str>,
    new_constraints: Option<Option<crate::models::database::table::UpdateTableConstraints>>,
    description: Option<Option<String>>,
    validate_existing_data: bool,
) -> Result<TableSummary, AppError> {
    if let Some(Some(desc)) = &description
        && desc.chars().count() > crate::models::database::MAX_DESCRIPTION_LENGTH
    {
        return Err(AppError::InvalidName {
            reason: format!(
                "Description cannot exceed {} characters",
                crate::models::database::MAX_DESCRIPTION_LENGTH
            ),
        });
    }

    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let new_name = new_name.map(str::to_string);

    let table = state
        .db
        .write(async move |txn| {
            txn.table_update(
                &db_name,
                &table_name,
                new_name.as_deref(),
                new_constraints,
                description,
                validate_existing_data,
            )
            .await
        })
        .await?;

    Ok(TableSummary {
        name: table.name,
        data_type: table.data_type,
        max_zoom_level: table.max_zoom_level,
        constraints: table.constraints,
        description: table.description,
    })
}
