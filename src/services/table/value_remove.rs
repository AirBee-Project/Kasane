use crate::{AppState, error::AppError, models::query::Query, repositories::write::SpatialDbWrite};

pub async fn value_remove(
    app_state: &AppState,
    table_name: &str,
    query: Query,
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

    //クエリの解釈と取得範囲の決定
    let ids = query.process(table.max_zoom_level)?;

    //データベースの操作と反映
    db.value_remove(table.id, ids)?;
    db.commit()
}
