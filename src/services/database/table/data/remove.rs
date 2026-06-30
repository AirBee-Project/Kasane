use crate::{
    AppState,
    error::AppError,
    models::{database::table::data::ZoomLevelPolicy, spatial_id::SpatialId},
};

pub async fn remove(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    app_state
        .db
        .batch_data_remove(
            db_name.to_string(),
            table_name.to_string(),
            spatial_ids.to_vec(),
            *zoom_level_policy,
        )
        .await
}
