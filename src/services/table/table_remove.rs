use crate::{AppState, error::AppError, repositories::write::SpatialDbWrite};

/// Services層でTableを削除する
pub async fn remove(app_state: &AppState, table_name: &str) -> Result<(), AppError> {
    let write_txn = app_state.redb.begin_write()?;
    let db = SpatialDbWrite::new(write_txn);

    //同名のTableが存在しない場合はエラーを返す
    if db.table_info(table_name)?.is_none() {
        return Err(AppError::TableNotFound {
            name: table_name.to_string(),
        });
    }

    //削除と反映
    db.table_remove(table_name)?;
    db.commit()?;

    Ok(())
}
