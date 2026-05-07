use crate::{AppState, error::AppError, repositories::layer::write::SpatialDbWrite};

/// Services層でLayerを削除する
pub async fn remove(app_state: &AppState, layer_name: &str) -> Result<(), AppError> {
    //データベースを開く
    let write_txn = app_state.redb.begin_write()?;
    let mut db = SpatialDbWrite::new(write_txn);

    //layerの削除と反映
    db.layer_remove(layer_name)?;
    db.commit()?;

    Ok(())
}
