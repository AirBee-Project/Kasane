use crate::{
    AppState, error::AppError, models::query::Query, repositories::table::write::SpatialDbWrite,
    services::helpers::value::interpret_value,
};

/// 空間IDの範囲を[Query]で指定して、値が存在しないIDにのみ値を書き込む（Upsert）
pub async fn value_upsert(
    app_state: &AppState,
    table_name: &str,
    query: Query,
    value: serde_json::Value,
) -> Result<(), AppError> {
    let write_txn = app_state.redb.begin_write()?;
    let db = SpatialDbWrite::new(write_txn);

    // Tableが存在することを検証
    let table = match db.table_info(table_name)? {
        Some(v) => v,
        None => {
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        }
    };

    // Valueの解釈
    let value = interpret_value(table.data_type, value)?;

    // クエリの解釈と取得範囲の決定
    let ids = query.process(table.max_zoom_level)?;

    // 値が存在しないIDにのみ書き込む（TODO: repository層で実装）
    crate::repositories::table::value::write::value_upsert(table.id, ids, &value)?;
    db.commit()
}
