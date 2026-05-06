use redb::ReadableDatabase;

use crate::{
    AppState,
    error::AppError,
    models::query::Query,
    models::value::GetValueResponse,
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

    let table = match db.table_info(table_name) {
        Ok(Some(v)) => v,
        Ok(None) => {
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        }
        Err(e) => {
            return Err(e);
        }
    };

    let ids = query.process(table.max_zoom_level)?;
    let mut result = vec![];
    for (single_id, value) in db.value_get(table.id, ids)? {
        result.push((single_id, restore_value(table.data_type, value)?));
    }

    Ok(GetValueResponse { ids: result })
}
