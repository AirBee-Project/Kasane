use crate::{
    AppState, error::AppError, models::table::TableDataType, repositories::write::SpatialDbWrite,
    services::helpers::name_valid::name_valid,
};

pub async fn create(
    app_state: &AppState,
    table_name: &str,
    data_type: TableDataType,
    max_zoom_level: u8,
) -> Result<(), AppError> {
    let write_txn = app_state.redb.begin_write()?;
    let db = SpatialDbWrite::new(write_txn);
    //すでに同名のTableが存在する場合はエラーを返す
    if db.table_info(table_name)?.is_some() {
        return Err(AppError::TableAlreadyExists {
            name: table_name.to_string(),
        });
    }
    //Table名のバリデーション
    let _ = name_valid(table_name)?;
    db.table_create(table_name, data_type, max_zoom_level)?;
    db.commit()?;
    Ok(())
}
