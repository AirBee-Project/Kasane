use redb::ReadableDatabase;

use crate::{
    AppState, error::AppError, models::table::InfoTableResponse, repositories::read::SpatialDbRead,
};

/// Services層からTableに関する情報を返す
pub async fn table_info(app_state: &AppState, name: &str) -> Result<InfoTableResponse, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = SpatialDbRead::new(read_txn);

    let table_metadata = match db.table_info(name) {
        Ok(result) => match result {
            Some(v) => v,
            None => {
                return Err(AppError::TableNotFound {
                    name: name.to_string(),
                });
            }
        },
        Err(e) => {
            return Err(e);
        }
    };

    Ok(InfoTableResponse {
        name: name.to_string(),
        r#type: table_metadata.r#type,
        max_zoom_level: table_metadata.max_zoom_level,
    })
}
