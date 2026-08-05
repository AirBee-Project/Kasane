use crate::{
    AppState, error::AppError, models::spatial_id::SpatialId,
    services::helpers::spatial_ids::to_spatial_id_set,
};

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn remove(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
) -> Result<(), AppError> {
    // 失敗し得るユーザ入力検証はバッチ投入前に済ませる（insert と同様）。
    let table = app_state
        .db
        .read(|r| r.table_info(db_name, table_name))?
        .ok_or_else(|| AppError::TableNotFound {
            name: table_name.to_string(),
        })?;

    let ids = to_spatial_id_set(spatial_ids)?;

    app_state
        .db
        .batch_data_remove(table.id, table.data_type, ids)
        .await
}
