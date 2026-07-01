use crate::{
    AppState,
    error::AppError,
    models::{database::table::data::ZoomLevelPolicy, spatial_id::SpatialId},
    services::helpers::spatial_ids::process_spatial_ids,
};

pub async fn remove(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    // 失敗し得るユーザ入力検証はバッチ投入前に済ませる（insert と同様）。
    let table = app_state
        .db
        .read(|r| r.table_info(db_name, table_name))?
        .ok_or_else(|| AppError::TableNotFound {
            name: table_name.to_string(),
        })?;

    let ids = process_spatial_ids(spatial_ids, table.max_zoom_level, zoom_level_policy)?;

    app_state
        .db
        .batch_data_remove(table.id, table.data_type, ids)
        .await
}
