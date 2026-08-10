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

    // トランザクションの内側で行うのは読み出しまで。
    //
    // レスポンス組み立て（値の復元と JSON 化）は FlexId 数に比例する CPU バウンド処理で、
    // ここに置くとバックエンドによって走る場所が変わってしまう。LMDB はクロージャ全体を
    // blocking タスク上で回すので問題ないが、TiKV は非同期ワーカー上で回すため、
    // 大きな結果ではワーカーを占有する。トランザクションの外へ出して明示的に
    // blocking タスクへ渡せば、どちらのバックエンドでも同じ扱いになる。
    let (table, groups) = app_state
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
            Ok((table, groups))
        })
        .await?;

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        span.in_scope(|| {
            data_response::build(groups, query_format, query_limit, |bytes| {
                restore_value(table.data_type, table.constraints.as_ref(), bytes)
            })
        })
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}
