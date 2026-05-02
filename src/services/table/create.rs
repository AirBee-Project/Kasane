use crate::{
    AppState, error::AppError, models::table::entity::TableMetadata,
    repositories::write::SpatialDbWrite, services::helpers::name_valid::name_valid,
};

pub async fn create(
    app_state: &AppState,
    name: &str,
    meta_data: TableMetadata,
) -> Result<(), AppError> {
    let write_txn = app_state.redb.begin_write()?;
    let db = SpatialDbWrite::new(write_txn);

    //すでに同名のTableが存在する場合はエラーを返す
    if db.table_info(name)?.is_some() {
        return Err(AppError::TableAlreadyExists {
            name: name.to_string(),
        });
    }

    //Table名のバリデーション
    let _ = name_valid(name)?;

    db.table_create(name, meta_data)?;
    db.commit()?;
    return Ok(());
}
