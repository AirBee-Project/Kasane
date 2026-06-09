use crate::{
    AppState,
    error::AppError,
    models::{database::table::data::ZoomLevelPolicy, query::Query},
    repositories::KasaneDbWrite,
    services::helpers::value::interpret_value,
};

/// 値が存在しないIDにのみ書き込む（Upsert）
pub async fn upsert(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    query: Query,
    value: serde_json::Value,
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    let write_txn = app_state.redb.begin_write()?;
    let db = KasaneDbWrite::new(write_txn);

    let table = match db.table_info(db_name, table_name)? {
        Some(v) => v,
        None => {
            tracing::debug!("Table not found: {}", table_name);
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        }
    };

    let value = interpret_value(table.data_type, value)?;
    let ids = query.process(table.max_zoom_level, zoom_level_policy)?;

    tracing::debug!("Upserting {} spatial IDs", ids.count());
    db.data_upsert(db_name, table_name, ids, &value)?;
    db.commit()?;
    Ok(())
}
