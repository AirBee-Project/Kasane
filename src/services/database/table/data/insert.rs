use crate::{
    AppState,
    error::AppError,
    models::spatial_id::SpatialId,
    services::helpers::{spatial_ids::to_spatial_id_set, value::interpret_value},
};

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn insert(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    value: serde_json::Value,
) -> Result<(), AppError> {
    // 失敗し得るユーザ入力検証（テーブル存在・値の解釈・ズーム解決）は、
    // 書き込みバッチへ投入する前に済ませておく。こうすることで、ある不正リクエストが
    // 同一バッチ内の無関係な正常リクエストを巻き添えにする問題を防ぐ。
    let table = app_state
        .db
        .read(|r| r.table_info(db_name, table_name))?
        .ok_or_else(|| AppError::TableNotFound {
            name: table_name.to_string(),
        })?;

    let value = interpret_value(table.data_type, table.constraints.as_ref(), value)?;

    let ids = to_spatial_id_set(spatial_ids)?;

    app_state
        .db
        .batch_data_insert(table.id, table.data_type, ids, value)
        .await
}
