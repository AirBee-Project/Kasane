use crate::{
    AppState,
    error::AppError,
    models::database::table::TableInfoResponse,
    models::users::{User, UserRole},
    repositories::{ReadRepository, Storage},
    services::helpers::authorize::resolve_authorized_table,
};

/// 認可・解決・件数取得を 1 回の読み取りで済ませる。
#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn info(
    app_state: &AppState,
    user: &User,
    db_name: &str,
    table_name: &str,
) -> Result<TableInfoResponse, AppError> {
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let user = user.clone();

    app_state
        .db
        .read(async move |db| {
            let table =
                resolve_authorized_table(db, &user, &db_name, &table_name, UserRole::Read).await?;
            Ok(TableInfoResponse {
                count: db.table_count(table.id).await?,
                name: table.name,
                data_type: table.data_type,
                max_zoom_level: table.max_zoom_level,
                constraints: table.constraints,
                description: table.description,
            })
        })
        .await
}
