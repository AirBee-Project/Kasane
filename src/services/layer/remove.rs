use crate::{AppState, error::AppError, repositories::layer::write::SpatialDbWrite};

/// Services層でLayerを削除する
pub async fn remove(app_state: &AppState, layer_name: &str) -> Result<(), AppError> {
    let write_txn = app_state.redb.begin_write()?;
    let db = SpatialDbWrite::new(write_txn);

    if db.layer_info(layer_name)?.is_none() {
        return Err(AppError::LayerNotFound {
            name: layer_name.to_string(),
        });
    }

    db.layer_remove(layer_name)?;
    db.commit()?;
    Ok(())
}
