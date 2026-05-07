use crate::{
    AppState, error::AppError, models::layer::LayerDataType,
    repositories::layer::write::SpatialDbWrite, services::helpers::name_valid::name_valid,
};

/// Services層でLayerを作成する
pub async fn create(
    app_state: &AppState,
    layer_name: &str,
    data_type: LayerDataType,
    max_zoom_level: u8,
) -> Result<(), AppError> {
    //データベースを開く
    let write_txn = app_state.redb.begin_write()?;
    let mut db = SpatialDbWrite::new(write_txn);

    //名前の検証
    let _ = name_valid(layer_name)?;

    //layerの作成と反映
    db.layer_create(layer_name, data_type, max_zoom_level)?;
    db.commit()?;

    Ok(())
}
