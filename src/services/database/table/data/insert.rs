use crate::{
    AppState,
    error::AppError,
    models::users::{User, UserRole},
    models::{database::table::data::ZoomLevelPolicy, spatial_id::SpatialId},
    services::helpers::{
        authorize::authorized_table,
        spatial_ids::process_spatial_ids,
        value::{enforce_max_stored_size, interpret_value},
    },
};

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn insert(
    app_state: &AppState,
    user: &User,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    value: serde_json::Value,
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    // 認可とメタデータ取得が同じ読み取りで済む。以前は認可で引いた名前を捨てて
    // ここで引き直していた。
    let table = authorized_table(app_state, user, db_name, table_name, UserRole::Write).await?;

    // 不正リクエストが同一バッチ内の正常なリクエストを巻き添えにしないため。
    let value = interpret_value(table.data_type, table.constraints.as_ref(), value)?;
    enforce_max_stored_size(&value)?;
    let ids = process_spatial_ids(
        spatial_ids,
        table.max_zoom_level,
        zoom_level_policy,
        !table.has_time,
    )?;

    // 1 件ずつ別トランザクションで書くと、同じリーフを何度も書き直しロックを奪い合う。
    app_state
        .writes
        .insert(table.id, table.value_indexing(), ids, value)
        .await
}
