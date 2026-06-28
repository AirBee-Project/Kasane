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

    // 値の復元はリポジトリの txn スコープ内・並列ワーカー内で行う
    // （生バイト列をコピーせず、その場で JSON 値へ復元する）。
    let data_type = table.data_type;
    let decoded = db.data_get(table.id.into(), ids, |bytes| restore_value(data_type, bytes))?;

    let mut result = Vec::with_capacity(decoded.len());
    for (single_id, json_value) in decoded {
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
