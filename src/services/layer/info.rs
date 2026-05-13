use redb::ReadableDatabase;

use crate::{
    AppState, error::AppError, models::layer::Layer, repositories::layer::read::SpatialDbRead,
};

/// Services層からLayerに関する情報を返す
pub async fn info(app_state: &AppState, layer_name: &str) -> Result<Layer, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = SpatialDbRead::new(read_txn);

    match db.layer_info(layer_name) {
        Ok(Some(layer)) => Ok(layer),
        Ok(None) => {
            tracing::debug!("Layer not found: {}", layer_name);
            Err(AppError::LayerNotFound {
                name: layer_name.to_string(),
            })
        }
        Err(e) => {
            tracing::error!("Failed to retrieve layer info for '{}': {}", layer_name, e);
            Err(e)
        }
    }
}
