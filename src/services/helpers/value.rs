//! 格納バイト列と値リテラル ([`ValueLiteral`]) の相互変換。
//!
//! 型ごとの符号化・復号・検証は [`crate::services::query::value::Value`] が一手に引き受け、
//! ここは `data_type` から具体型へ単型化して trait を呼ぶだけの薄い入口。`search`（復元）も
//! `insert`/`upsert`（格納）も同じ trait を通るので、型ごとの処理を二重に持たない。

use crate::{
    error::{AppError, Stored},
    for_value_type,
    models::{
        ValueLiteral,
        database::table::{TableConstraints, TableDataType},
    },
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

/// 値リテラルを、テーブルのデータ型に基づいて解釈し、格納バイト列へ変換する（制約検証込み）。
///
/// **サイズ上限（[`MAX_STORED_VALUE_BYTES`]）はここでは検証しない。** この関数はテーブルの
/// 制約変更時の既存データ再検証（`validate_existing_data`）からも呼ばれる。上限は「これから
/// 新しく書き込む値」に対する保存エンジン側の安全装置であり、テーブルが宣言する制約とは別物
/// なので、ここに混ぜると「上限を設ける前から格納されていた値」を持つテーブルで、サイズと
/// 無関係な制約変更（例えば Int の min/max だけの変更）まで失敗するようになってしまう。
/// 新規書き込み経路（`services::database::table::data::insert` / `upsert`）は、この関数の
/// 直後に必ず [`enforce_max_stored_size`] を呼ぶこと。
pub fn interpret_value(
    expected_type: TableDataType,
    constraints: Option<&TableConstraints>,
    value: ValueLiteral,
) -> Result<Vec<u8>, AppError> {
    fn imp<V: Value>(
        value: &ValueLiteral,
        constraints: Option<&TableConstraints>,
    ) -> Result<Vec<u8>, AppError> {
        V::from_value(value)?.encode(constraints)
    }
    for_value_type!(expected_type, imp, &value, constraints)
}

/// 新規に書き込む値のバイト数が [`MAX_STORED_VALUE_BYTES`] を超えていないか検証する。
///
/// [`interpret_value`] とは別関数にしてある理由はそちらのドキュメントを参照。
pub fn enforce_max_stored_size(encoded: &[u8]) -> Result<(), AppError> {
    if encoded.len() > MAX_STORED_VALUE_BYTES {
        return Err(AppError::ConstraintViolation {
            reason: format!(
                "encoded value is {} bytes, which exceeds the maximum of {} bytes",
                encoded.len(),
                MAX_STORED_VALUE_BYTES
            ),
        });
    }
    Ok(())
}

/// 格納バイト列を、テーブルのデータ型に基づいて [`ValueLiteral`] へ復元する。
pub fn restore_value(
    expected_type: TableDataType,
    constraints: Option<&TableConstraints>,
    value: &[u8],
) -> Result<ValueLiteral, AppError> {
    fn imp<V: Value>(
        bytes: &[u8],
        constraints: Option<&TableConstraints>,
    ) -> Result<ValueLiteral, AppError> {
        let decode = V::decoder(constraints)?;
        decode(bytes).map(|v| v.to_value()).ok_or_else(|| {
            AppError::corrupt(
                Stored::Value,
                format!("bytes are not a valid {}", V::type_name()),
            )
        })
    }
    for_value_type!(expected_type, imp, value, constraints)
}
