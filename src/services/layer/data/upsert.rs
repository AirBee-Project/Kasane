use crate::{
    AppState,
    error::AppError,
    models::{layer::data::ZoomLevelPolicy, query::Query},
    repositories::layer::write::SpatialDbWrite,
    services::helpers::value::interpret_value,
};

/// 値が存在しないIDにのみ書き込む（Upsert）
pub async fn upsert(
    app_state: &AppState,
    layer_name: &str,
    query: Query,
    value: serde_json::Value,
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

    let value = interpret_value(layer.data_type, value)?;
    let ids = query.process(layer.max_zoom_level, zoom_level_policy)?;

    tracing::debug!("Upserting {} spatial IDs", ids.len());
    db.data_upsert(layer_name, ids, &value)?;
    db.commit()?;
    Ok(())
}
