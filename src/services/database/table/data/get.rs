use crate::{
    AppState,
    error::AppError,
    models::{
        database::table::data::{GetDataResponse, SpatialData, ZoomLevelPolicy},
        query::Query,
        spatial_id::RawSingleId,
    },
    repositories::KasaneDbRead,
    services::helpers::value::restore_value,
};
use redb::ReadableDatabase;

pub async fn get(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    query: Query,
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<GetDataResponse, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = KasaneDbRead::new(read_txn);
    let table = match db.table_info(db_name, table_name) {
        Ok(Some(v)) => v,
        Ok(None) => {
            tracing::debug!("Table not found: {}", table_name);
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        }
        Err(e) => {
            tracing::error!("Failed to get table info for '{}': {}", table_name, e);
            return Err(e);
        }
    };
    let ids = query.process(table.max_zoom_level, zoom_level_policy)?;
    tracing::debug!("Searching {} spatial IDs", ids.count());

    let mut result = Vec::new();
    for (single_id, value) in db.data_get(db_name, table_name, ids)? {
        let json_value = restore_value(table.data_type, &value)?;
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
