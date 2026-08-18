use crate::{
    AppState,
    error::AppError,
    models::users::{User, UserRole},
    models::{database::table::data::ZoomLevelPolicy, spatial_id::SpatialId},
    repositories::{Storage, WriteRepository},
    services::helpers::{authorize::authorized_table, spatial_ids::process_spatial_ids},
};

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn remove(
    app_state: &AppState,
    user: &User,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    let table = authorized_table(app_state, user, db_name, table_name, UserRole::Write).await?;

    // 失敗し得るユーザ入力検証はバッチ投入前に済ませる（insert と同様）。
    let ids = process_spatial_ids(
        spatial_ids,
        table.max_zoom_level,
        zoom_level_policy,
        !table.has_time,
    )?;

    app_state
        .db
        .write(async move |w| {
            w.data_remove(table.id, table.value_indexing(), ids.clone())
                .await
        })
        .await
}
