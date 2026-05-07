use redb::ReadableDatabase;

use crate::{
    AppState,
    error::AppError,
    models::{
        layer::data::{GetDataResponse, ResponseSpatialId, SpatialData},
        query::Query,
    },
    repositories::layer::read::SpatialDbRead,
    services::helpers::value::restore_value,
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
        Ok(None) => {
            return Err(AppError::LayerNotFound {
                name: layer_name.to_string(),
            });
        }
        Err(e) => return Err(e),
    };
    let ids = query.process(layer.max_zoom_level)?;
    let mut result = Vec::new();
    for (single_id, value) in db.data_get(layer_name, ids)? {
        let json_value = restore_value(layer.data_type, &value)?;
        result.push(SpatialData {
            id: ResponseSpatialId::SingleId(single_id),
            data: json_value,
        });
    }
    Ok(GetDataResponse { ids: result })
}
