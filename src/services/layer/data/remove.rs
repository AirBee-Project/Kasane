use crate::{
    AppState,
    error::AppError,
    models::{layer::data::ZoomLevelPolicy, query::Query},
    repositories::layer::write::SpatialDbWrite,
};
pub async fn remove(
    app_state: &AppState,
    layer_name: &str,
    query: Query,
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    let write_txn = app_state.redb.begin_write()?;
    let db = SpatialDbWrite::new(write_txn);

    let layer = match db.layer_info(layer_name)? {
        Some(v) => v,
        None => {
            tracing::debug!("Layer not found: {}", layer_name);
            return Err(AppError::LayerNotFound {
                name: layer_name.to_string(),
            });
        }
    };

    let ids = query.process(layer.max_zoom_level, zoom_level_policy)?;
    tracing::debug!("Removing {} spatial IDs", ids.count());
    db.data_remove(layer_name, ids)?;
    db.commit()?;
    Ok(())
}
