use redb::ReadableDatabase;

use crate::{
    AppState,
    error::AppError,
    models::{
        layer::data::{GetDataResponse, SpatialData, ZoomLevelPolicy},
        query::Query,
        spatial_id::RawSingleId,
    },
    repositories::layer::read::SpatialDbRead,
    services::helpers::value::restore_value,
};

pub async fn get(
    app_state: &AppState,
    layer_name: &str,
    query: Query,
    zoom_level_policy: &ZoomLevelPolicy,
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
    let ids = query.process(layer.max_zoom_level, zoom_level_policy)?;
    let mut result = Vec::new();
    for (single_id, value) in db.data_get(layer_name, ids)? {
        let json_value = restore_value(layer.data_type, &value)?;
        result.push(SpatialData {
            id: RawSingleId {
                z: single_id.z(),
                f: single_id.f(),
                x: single_id.x(),
                y: single_id.y(),
            },
            data: json_value,
        });
    }
    Ok(GetDataResponse { ids: result })
}
