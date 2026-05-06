use redb::ReadableDatabase;

use crate::{
    AppState,
    error::AppError,
    models::table::{Query, response::GetValueResponse},
    repositories::read::SpatialDbRead,
    services::helpers::value::restore_value,
};

pub async fn get(
    app_state: &AppState,
    table_name: &str,
    query: Query,
) -> Result<GetValueResponse, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let db = SpatialDbRead::new(read_txn);

    let table_metadata = match db.table_info(table_name) {
        Ok(result) => match result {
            Some(v) => v,
            None => {
                return Err(AppError::TableNotFound {
                    name: table_name.to_string(),
                });
            }
        },
        Err(e) => {
            return Err(e);
        }
    };

    let ids = query.process(table_metadata.max_zoom_level)?;
    let mut result = vec![];
    for (single_id, value) in db.spatial_get(table_metadata.rank, ids)? {
        result.push((single_id, restore_value(table_metadata.r#type, value)?));
    }

    Ok(GetValueResponse { ids: result })
}
