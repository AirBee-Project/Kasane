use crate::{
    AppState,
    error::AppError,
    models::{database::table::data::ZoomLevelPolicy, spatial_id::SpatialId},
};

/// 値が存在しないIDにのみ書き込む（Upsert）
pub async fn upsert(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    value: serde_json::Value,
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    app_state
        .db
        .batch_data_upsert(
            db_name.to_string(),
            table_name.to_string(),
            spatial_ids.to_vec(),
            *zoom_level_policy,
            value,
        )
        .await
}
