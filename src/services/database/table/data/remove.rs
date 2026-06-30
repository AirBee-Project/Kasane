use crate::{
    AppState,
    error::AppError,
    models::{database::table::data::ZoomLevelPolicy, spatial_id::SpatialId},
    repositories::KasaneDbWrite,
    services::helpers::spatial_ids::process_spatial_ids,
};

pub async fn remove(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    let app_state = app_state.clone();
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let spatial_ids = spatial_ids.to_vec();
    let zoom_level_policy = *zoom_level_policy;

    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let write_txn = app_state.db.env.write_txn()?;
        let mut db = KasaneDbWrite::new(write_txn, &app_state.db);

        let table = match db.table_info(&db_name, &table_name)? {
            Some(v) => v,
            None => {
                tracing::debug!("Table not found: {}", table_name);
                return Err(AppError::TableNotFound {
                    name: table_name.to_string(),
                });
            }
        };

        let ids = process_spatial_ids(&spatial_ids, table.max_zoom_level, &zoom_level_policy)?;
        tracing::debug!("Removing {} spatial IDs", ids.count());
        db.data_remove(table.id, table.data_type, ids)?;
        db.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}
