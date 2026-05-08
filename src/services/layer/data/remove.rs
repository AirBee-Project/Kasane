use crate::{
    AppState, error::AppError, models::query::Query, repositories::layer::write::SpatialDbWrite,
};

pub async fn remove(app_state: &AppState, layer_name: &str, query: Query) -> Result<(), AppError> {
    let write_txn = app_state.redb.begin_write()?;
    let db = SpatialDbWrite::new(write_txn);

    let layer = match db.layer_info(layer_name)? {
        Some(v) => v,
        None => {
            return Err(AppError::LayerNotFound {
                name: layer_name.to_string(),
            });
        }
    };

    let _ids = query.process(layer.max_zoom_level)?;

    // db.data
    todo!();

    // db.commit()
}
