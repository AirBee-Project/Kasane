use kasane_logic::spatial_id::collection::flex_tree::table;

use crate::{
    AppState, error::AppError, models::table::Query, repositories::write::SpatialDbWrite,
    services::helpers::name_valid::name_valid,
};

pub async fn insert(
    app_state: &AppState,
    table_name: &str,
    query: Query,
    value: serde_json::Value,
) -> Result<(), AppError> {
    let write_txn = app_state.redb.begin_write()?;
    let db = SpatialDbWrite::new(write_txn);

    //Tableが存在することを検証
    let table_meta = match db.table_info(table_name)? {
        Some(v) => v,
        None => {
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        }
    };

    todo!()
}
