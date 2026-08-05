use crate::{AppState, error::AppError, models::database::table::TableSummary};

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn table_update(
    state: AppState,
    db_name: &str,
    table_name: &str,
    new_name: Option<&str>,
    new_constraints: Option<Option<crate::models::database::table::UpdateTableConstraints>>,
    validate_existing_data: bool,
) -> Result<TableSummary, AppError> {
    let table = state.db.write(|txn| {
        txn.table_update(
            db_name,
            table_name,
            new_name,
            new_constraints,
            validate_existing_data,
        )
    })?;

    Ok(TableSummary {
        name: table.name,
        data_type: table.data_type,
        constraints: table.constraints,
    })
}
