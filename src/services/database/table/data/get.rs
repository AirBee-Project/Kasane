use crate::{
    AppState,
    error::AppError,
    models::{
        database::table::data::{GetDataQuery, GetDataResponse, ZoomLevelPolicy},
        spatial_id::SpatialId,
    },
    services::helpers::{data_response, spatial_ids::process_spatial_ids, value::restore_value},
};

pub async fn get(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    zoom_level_policy: &ZoomLevelPolicy,
    query: &GetDataQuery,
) -> Result<GetDataResponse, AppError> {
    let app_state = app_state.clone();
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let spatial_ids = spatial_ids.to_vec();
    let zoom_level_policy = *zoom_level_policy;
    let query_format = query.format;
    let query_limit = query.limit;

    tokio::task::spawn_blocking(move || {
        let (data_type, constraints, groups) = app_state.db.read(|db| {
            let table = match db.table_info(&db_name, &table_name) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    tracing::debug!("Table not found: {}", table_name);
                    return Err(AppError::TableNotFound {
                        name: table_name.clone(),
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to get table info for '{}': {}", table_name, e);
                    return Err(e);
                }
            };
            let ids = process_spatial_ids(&spatial_ids, table.max_zoom_level, &zoom_level_policy)?;
            tracing::debug!("Searching {} spatial IDs", ids.count());

            let groups = db.data_get(table.id, ids)?;
            Ok((table.data_type, table.constraints, groups))
        })?;

        data_response::build(groups, query_format, query_limit, |bytes| {
            restore_value(data_type, constraints.as_ref(), bytes)
        })
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}
