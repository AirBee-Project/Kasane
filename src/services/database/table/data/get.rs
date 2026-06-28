use crate::{
    AppState,
    error::AppError,
    models::{
        database::table::data::{GetDataResponse, SpatialData, ZoomLevelPolicy},
        spatial_id::{RawSingleId, SpatialId},
    },
    repositories::KasaneDbRead,
    services::helpers::{spatial_ids::process_spatial_ids, value::restore_value},
};
use kasane_logic::IntoSingleIds;

pub async fn get(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<GetDataResponse, AppError> {
    let read_txn = app_state.db.env.read_txn()?;
    let db = KasaneDbRead::new(read_txn, &app_state.db);
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
    let ids = process_spatial_ids(spatial_ids, table.max_zoom_level, zoom_level_policy)?;
    tracing::debug!("Searching {} spatial IDs", ids.count());

    let data_type = table.data_type;
    let groups = db.data_get(table.id, ids)?;

    // data_get は (値バイト, FlexId群) を返す。値の復元は値ごとに1回、
    // SingleId への展開は上位レイヤー（ここ）で行う。
    let mut result = Vec::new();
    for (bytes, flex_ids) in groups {
        let json_value = restore_value(data_type, &bytes)?;
        for flex_id in flex_ids {
            for single_id in flex_id.into_single_ids() {
                result.push(SpatialData {
                    id: RawSingleId {
                        z: single_id.z(),
                        f: single_id.f(),
                        x: single_id.x(),
                        y: single_id.y(),
                    },
                    data: json_value.clone(),
                });
            }
        }
    }
    Ok(GetDataResponse { ids: result })
}
