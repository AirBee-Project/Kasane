use crate::{
    AppState,
    error::AppError,
    models::{database::table::data::ZoomLevelPolicy, spatial_id::SpatialId},
    repositories::KasaneDbWrite,
    services::helpers::{spatial_ids::process_spatial_ids, value::interpret_value},
};

pub async fn insert(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    value: serde_json::Value,
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

    let value = interpret_value(table.data_type, value)?;
    let ids = process_spatial_ids(spatial_ids, table.max_zoom_level, zoom_level_policy)?;

    tracing::debug!("Inserting {} spatial IDs", ids.count());
    db.data_insert(db_name, table_name, ids, &value)?;
    db.commit()?;
    Ok(())
}
