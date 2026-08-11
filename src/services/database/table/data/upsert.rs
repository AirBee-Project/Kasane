use crate::{
    AppState,
    error::AppError,
    models::{database::table::data::ZoomLevelPolicy, spatial_id::SpatialId},
    repositories::{ReadRepository, Storage, WriteRepository},
    services::helpers::{spatial_ids::process_spatial_ids, value::interpret_value},
};

/// 値が存在しないIDにのみ書き込む（Upsert）
#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn upsert(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    value: serde_json::Value,
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    // 失敗し得るユーザ入力検証はバッチ投入前に済ませる（insert と同様）。
    let owned_db = db_name.to_string();
    let owned_table = table_name.to_string();
    let table = app_state
        .db
        .read(async move |r| r.table_info(&owned_db, &owned_table).await)
        .await?
        .ok_or_else(|| AppError::TableNotFound {
            name: table_name.to_string(),
        })?;

    let value = interpret_value(table.data_type, table.constraints.as_ref(), value)?;

    let ids = process_spatial_ids(spatial_ids, table.max_zoom_level, zoom_level_policy)?;

    app_state
        .db
        .write(async move |w| {
            w.data_upsert(table.id, table.value_indexing(), ids, &value)
                .await
        })
        .await
}
