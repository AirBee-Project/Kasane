use crate::{
    AppState, error::AppError, models::query::Query, repositories::write::SpatialDbWrite,
    services::helpers::value::interpret_value,
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
    let table = match db.table_info(table_name)? {
        Some(v) => v,
        None => {
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        }
    };

    //Valueの解釈
    let value = interpret_value(table.data_type, value)?;

    //クエリの解釈と取得範囲の決定
    let ids = query.process(table.max_zoom_level)?;

    //データベースの操作と反映
    db.value_insert(table.id, ids, &value)?;
    db.commit()
}
