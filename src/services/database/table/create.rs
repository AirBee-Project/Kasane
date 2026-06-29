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

    // max_zoom_levelの検証
    kasane_logic::ZoomLevel::new(req.max_zoom_level)?;

    let write_txn = app_state.db.env.write_txn()?;
    let mut db = crate::repositories::KasaneDbWrite::new(write_txn, &app_state.db);
    let res = db.table_create(db_name, table_name, req.data_type, req.max_zoom_level)?;
    db.commit()?;
    Ok(res)
}
