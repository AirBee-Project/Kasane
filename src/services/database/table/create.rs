use crate::{
    AppState,
    error::AppError,
    models::database::table::{CreateTableRequest, Table},
};

pub async fn create(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    req: CreateTableRequest,
) -> Result<Table, AppError> {
    crate::services::helpers::name_valid::name_valid(table_name)?;

    let write_txn = app_state.redb.begin_write()?;
    let mut db = crate::repositories::KasaneDbWrite::new(write_txn);
    let res = db.table_create(db_name, table_name, req.data_type, req.max_zoom_level)?;
    db.commit()?;
    Ok(res)
}
