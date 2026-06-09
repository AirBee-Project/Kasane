use crate::{
    AppState,
    error::AppError,
    models::{database::table::data::ZoomLevelPolicy, query::Query},
    repositories::KasaneDbWrite,
};

pub async fn remove(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    query: Query,
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

    let ids = query.process(table.max_zoom_level, zoom_level_policy)?;
    tracing::debug!("Removing {} spatial IDs", ids.count());
    db.data_remove(db_name, table_name, ids)?;
    db.commit()?;
    Ok(())
}
