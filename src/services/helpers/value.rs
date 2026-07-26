//! 格納バイト列と JSON の相互変換。
//!
//! 型ごとの符号化・復号・検証は [`crate::services::query::value::Value`] が一手に引き受け、
//! ここは `data_type` から具体型へ単型化して trait を呼ぶだけの薄い入口。`search`（復元）も
//! `insert`/`upsert`（格納）も同じ trait を通るので、型ごとの処理を二重に持たない。

use crate::{
    error::AppError,
    for_value_type,
    models::database::table::{TableConstraints, TableDataType},
    services::query::value::Value,
};

/// JSON の値を、テーブルのデータ型に基づいて解釈し、格納バイト列へ変換する（制約検証込み）。
pub fn interpret_value(
    expected_type: TableDataType,
    constraints: Option<&TableConstraints>,
    value: serde_json::Value,
) -> Result<Vec<u8>, AppError> {
    fn imp<V: Value>(
        value: &serde_json::Value,
        constraints: Option<&TableConstraints>,
    ) -> Result<Vec<u8>, AppError> {
        V::from_json(value)?.encode(constraints)
    }
    for_value_type!(expected_type, imp, &value, constraints)
}

/// 格納バイト列を、テーブルのデータ型に基づいて JSON 値へ復元する。
pub fn restore_value(
    expected_type: TableDataType,
    constraints: Option<&TableConstraints>,
    value: &[u8],
) -> Result<serde_json::Value, AppError> {
    fn imp<V: Value>(
        bytes: &[u8],
        constraints: Option<&TableConstraints>,
    ) -> Result<serde_json::Value, AppError> {
        let decode = V::decoder(constraints)?;
        decode(bytes)
            .map(|v| v.to_json())
            .ok_or_else(|| AppError::InvalidStoredValue {
                reason: format!("stored bytes are not a valid {}", V::type_name()),
            })
    }
    for_value_type!(expected_type, imp, value, constraints)
}
