use redb::ReadableDatabase;

use crate::{
    AppState, error::AppError, models::query::Query, models::layer::data::GetDataResponse,
    repositories::layer::read::SpatialDbRead, services::helpers::value::restore_value,
};

pub async fn get(
    app_state: &AppState,
    layer_name: &str,
    query: Query,
) -> Result<GetDataResponse, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = SpatialDbRead::new(read_txn);

    let layer = match db.layer_info(layer_name) {
        Ok(Some(v)) => v,
        Ok(None) => return Err(AppError::LayerNotFound { name: layer_name.to_string() }),
        Err(e) => return Err(e),
    };

    let ids = query.process(layer.max_zoom_level)?;
    let data_db = crate::repositories::layer::data::read::SpatialDbRead::new(
        app_state.redb.begin_read()?,
    );
    let mut result = vec![];
    for (single_id, value) in data_db.data_get(layer.id, ids)? {
        result.push((single_id, restore_value(layer.data_type, value)?));
    }

    Ok(GetDataResponse { ids: result })
}
