use crate::{
    AppState,
    error::AppError,
    models::{
        database::table::data::{GetDataQuery, GetDataResponse, ZoomLevelPolicy},
        spatial_id::SpatialId,
    },
    repositories::{ReadRepository, Storage},
    services::helpers::{data_response, spatial_ids::process_spatial_ids, value::restore_value},
};

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn get(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    zoom_level_policy: &ZoomLevelPolicy,
    query: &GetDataQuery,
) -> Result<GetDataResponse, AppError> {
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let spatial_ids = spatial_ids.to_vec();
    let zoom_level_policy = *zoom_level_policy;
    let query_format = query.format;
    let query_limit = query.limit;

    // レスポンス組み立てまでトランザクションの内側で行う。LMDB ではこのクロージャ全体が
    // 1 つの blocking タスク上で回るため、CPU バウンドな復元処理も非同期ランタイムを塞がない。
    app_state
        .db
        .read(async move |db| {
            let table = match db.table_info(&db_name, &table_name).await {
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

            let ids = process_spatial_ids(&spatial_ids, table.max_zoom_level, &zoom_level_policy)?;
            let groups = db.data_get(table.id, ids, query_limit).await?;

            data_response::build(groups, query_format, query_limit, |bytes| {
                restore_value(table.data_type, table.constraints.as_ref(), bytes)
            })
        })
        .await
}
