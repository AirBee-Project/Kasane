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
    let write_txn = app_state.redb.begin_write()?;
    let db = KasaneDbWrite::new(write_txn);

    let table = match db.table_info(db_name, table_name)? {
        Some(v) => v,
        None => {
            tracing::debug!("Table not found: {}", table_name);
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        }
    };

    let ids = process_spatial_ids(spatial_ids, table.max_zoom_level, zoom_level_policy)?;
    tracing::debug!("Removing {} spatial IDs", ids.count());
    db.data_remove(table.id, ids)?;
    db.commit()?;
    Ok(())
}
