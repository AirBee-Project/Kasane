use crate::{
    AppState,
    error::AppError,
    middleware::auth::authorize_path,
    models::database::table::TableSummary,
    models::users::{User, UserRole},
    repositories::{Storage, WriteRepository},
};

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn table_update(
    state: AppState,
    user: &User,
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
    let user = user.clone();

    let table = state
        .db
        .write(async move |txn| {
            // 権限はテーブル ID に紐づくので、改名しても権限はそのテーブルに追従する。
            authorize_path(txn, &user, &db_name, Some(&table_name), UserRole::Manage).await?;
            txn.table_update(
                &db_name,
                &table_name,
                new_name.as_deref(),
                new_constraints.clone(),
                description.clone(),
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
