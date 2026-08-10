use crate::{
    AppState,
    error::AppError,
    models::database::table::TableInfoResponse,
    repositories::{ReadRepository, Storage},
};

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn info(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
) -> Result<TableInfoResponse, AppError> {
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    app_state
        .db
        .read(
            async move |db| match db.table_info(&db_name, &table_name).await? {
                Some(table) => {
                    let count = db.table_count(table.id).await?;
                    Ok(TableInfoResponse {
                        name: table.name,
                        data_type: table.data_type,
                        max_zoom_level: table.max_zoom_level,
                        count,
                        constraints: table.constraints,
                        description: table.description,
                    })
                }
                None => Err(AppError::TableNotFound { name: table_name }),
            },
        )
        .await
}
