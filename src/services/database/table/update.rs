use crate::{
    AppState, error::AppError, models::database::table::TableSummary, repositories::KasaneDbWrite,
};

pub async fn table_update(
    state: AppState,
    db_name: &str,
    table_name: &str,
    new_name: Option<&str>,
    new_constraints: Option<Option<crate::models::database::table::UpdateTableConstraints>>,
    validate_existing_data: bool,
) -> Result<TableSummary, AppError> {
    let mut txn = KasaneDbWrite::new(
        state.db.env.write_txn().map_err(AppError::StorageError)?,
        &state.db,
    );

    let table = txn.table_update(
        db_name,
        table_name,
        new_name,
        new_constraints,
        validate_existing_data,
    )?;

    txn.commit()?;

    Ok(TableSummary {
        name: table.name,
        data_type: table.data_type,
        max_zoom_level: table.max_zoom_level,
        constraints: table.constraints,
    })
}
