//! 格納バイト列と JSON の相互変換。
//!
//! 型ごとの符号化・復号・検証は [`crate::services::query::value::Value`] が一手に引き受け、
//! ここは `data_type` から具体型へ単型化して trait を呼ぶだけの薄い入口。`search`（復元）も
//! `insert`/`upsert`（格納）も同じ trait を通るので、型ごとの処理を二重に持たない。

use crate::{
    error::{AppError, Stored},
    for_value_type,
    models::database::table::{TableConstraints, TableDataType},
    services::query::value::Value,
};

/// 格納 1 件あたりの許容バイト数の上限。
///
/// 1 つの値は挿入対象の空間 ID 1 件につき 1 回複製されて葉へ書かれるので、上限を設けないと
/// 空間的にまとまった ID 群への一括挿入だけで、葉のバイト数上限
/// （[`crate::repositories::encoding::shard_entry::MAX_SHARD_BYTES`]）をたった 1 件の値で
/// 超えうる。件数 1 の葉は幾何分割してもバイト数が縮まらないため、ここでの上限がその
/// 前提（件数 1 の葉は必ず上限内に収まる）を保証している。
pub const MAX_STORED_VALUE_BYTES: usize = 256 * 1024;

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
    let encoded = for_value_type!(expected_type, imp, &value, constraints)?;
    if encoded.len() > MAX_STORED_VALUE_BYTES {
        return Err(AppError::ConstraintViolation {
            reason: format!(
                "encoded value is {} bytes, which exceeds the maximum of {} bytes",
                encoded.len(),
                MAX_STORED_VALUE_BYTES
            ),
        });
    }
    Ok(encoded)
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
        decode(bytes).map(|v| v.to_json()).ok_or_else(|| {
            AppError::corrupt(
                Stored::Value,
                format!("bytes are not a valid {}", V::type_name()),
            )
        })
    }
    for_value_type!(expected_type, imp, value, constraints)
}
