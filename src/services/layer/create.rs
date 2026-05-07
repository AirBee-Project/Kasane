use crate::{
    AppState, error::AppError, models::layer::LayerDataType,
    repositories::layer::write::SpatialDbWrite,
    services::helpers::name_valid::name_valid,
};

/// Services層でLayerを作成する
pub async fn create(
    app_state: &AppState,
    layer_name: &str,
    data_type: LayerDataType,
    max_zoom_level: u8,
) -> Result<(), AppError> {
    let write_txn = app_state.redb.begin_write()?;
    let db = SpatialDbWrite::new(write_txn);

    if db.layer_info(layer_name)?.is_some() {
        return Err(AppError::LayerAlreadyExists {
            name: layer_name.to_string(),
        });
    }
    let _ = name_valid(layer_name)?;

    db.layer_create(layer_name, data_type, max_zoom_level)?;
    db.commit()?;
    Ok(())
}
