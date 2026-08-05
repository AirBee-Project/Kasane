use crate::{
    AppState,
    error::AppError,
    models::{
        database::table::data::{GetDataQuery, GetDataResponse},
        spatial_id::SpatialId,
    },
    services::helpers::{data_response, spatial_ids::to_spatial_id_set, value::restore_value},
};

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn get(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    query: &GetDataQuery,
) -> Result<GetDataResponse, AppError> {
    let app_state = app_state.clone();
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let spatial_ids = spatial_ids.to_vec();
    let query_format = query.format;
    let query_limit = query.limit;

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        span.in_scope(|| {
            let (data_type, constraints, groups) = app_state.db.read(|db| {
                let table = match db.table_info(&db_name, &table_name) {
                    Ok(Some(v)) => Ok(v),
                    Ok(None) => {
                        tracing::debug!("Table not found: {}", table_name);
                        Err(AppError::TableNotFound {
                            name: table_name.clone(),
                        })
                    }
                    Err(e) => {
                        tracing::error!("Failed to get table info for '{}': {}", table_name, e);
                        Err(e)
                    }
                }?;

                let ids = to_spatial_id_set(&spatial_ids)?;

                let groups = db.data_get(table.id, ids)?;

                Ok((table.data_type, table.constraints, groups))
            })?;

            data_response::build(groups, query_format, query_limit, |bytes| {
                restore_value(data_type, constraints.as_ref(), bytes)
            })
        })
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}
