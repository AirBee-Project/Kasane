use std::convert::TryFrom;

use crate::{
    error::AppError,
    models::database::table::{JsonValueType, TableConstraints, TableDataType},
};

fn check_numeric_constraint<T: PartialOrd + std::fmt::Display>(
    value: T,
    min: Option<T>,
    max: Option<T>,
) -> Result<(), AppError> {
    if let Some(min_val) = min
        && value < min_val {
            return Err(AppError::ConstraintViolation {
                reason: format!("値 {} は最小値 {} を下回っています", value, min_val),
            });
        }
    if let Some(max_val) = max
        && value > max_val {
            return Err(AppError::ConstraintViolation {
                reason: format!("値 {} は最大値 {} を上回っています", value, max_val),
            });
        }
    Ok(())
}

macro_rules! parse_and_validate_int {
    ($number:expr, $constraints:expr, $constraint_variant:path, $rust_type:ty) => {{
        let v_i64 = $number
            .as_i64()
            .ok_or_else(|| AppError::NumericValueOutOfRange {
                actual: $number.to_string(),
                expected: stringify!($rust_type).to_string(),
            })?;
        let v = <$rust_type>::try_from(v_i64).map_err(|_| AppError::NumericValueOutOfRange {
            actual: $number.to_string(),
            expected: stringify!($rust_type).to_string(),
        })?;
        if let Some($constraint_variant { min, max }) = $constraints {
            check_numeric_constraint(v, *min, *max)?;
        }
        Ok(v.to_be_bytes().to_vec())
    }};
}

macro_rules! parse_and_validate_float {
    ($number:expr, $constraints:expr, $constraint_variant:path, $rust_type:ty) => {{
        let v_f64 = $number
            .as_f64()
            .ok_or_else(|| AppError::NumericValueOutOfRange {
                actual: $number.to_string(),
                expected: stringify!($rust_type).to_string(),
            })?;
        let v = v_f64 as $rust_type;
        if !v.is_finite() {
            return Err(AppError::NumericValueOutOfRange {
                actual: $number.to_string(),
                expected: stringify!($rust_type).to_string(),
            });
        }
        if let Some($constraint_variant { min, max }) = $constraints {
            check_numeric_constraint(v, *min, *max)?;
        }
        Ok(v.to_be_bytes().to_vec())
    }};
}

macro_rules! restore_int {
    ($bytes:expr, $rust_type:ty) => {{
        let size = std::mem::size_of::<$rust_type>();
        if $bytes.len() != size {
            return Err(AppError::InvalidStoredValue {
                reason: format!("int value must be {} bytes, got {}", size, $bytes.len()),
            });
        }
        let mut b = [0u8; std::mem::size_of::<$rust_type>()];
        b.copy_from_slice($bytes);
        let v = <$rust_type>::from_be_bytes(b);
        Ok(serde_json::Value::Number(serde_json::Number::from(
            v as i64,
        )))
    }};
}

macro_rules! restore_float {
    ($bytes:expr, $rust_type:ty) => {{
        let size = std::mem::size_of::<$rust_type>();
        if $bytes.len() != size {
            return Err(AppError::InvalidStoredValue {
                reason: format!("float value must be {} bytes, got {}", size, $bytes.len()),
            });
        }
        let mut b = [0u8; std::mem::size_of::<$rust_type>()];
        b.copy_from_slice($bytes);
        let v = <$rust_type>::from_be_bytes(b);
        let json_number =
            serde_json::Number::from_f64(v as f64).ok_or_else(|| AppError::InvalidStoredValue {
                reason: "float value cannot be represented as JSON number".to_string(),
            })?;
        Ok(serde_json::Value::Number(json_number))
    }};
}

/// JSONの値を、レイヤーのデータ型に基づいて解釈し、バイト列に変換する関数
pub fn interpret_value(
    expected_type: TableDataType,
    constraints: Option<&TableConstraints>,
    value: serde_json::Value,
) -> Result<Vec<u8>, AppError> {
    match value {
        serde_json::Value::Null => {
            if expected_type == TableDataType::Presence {
                Ok(vec![])
            } else {
                Err(AppError::ValueTypeMismatch {
                    actual: JsonValueType::Null,
                    expected: expected_type.into(),
                })
            }
        }
        serde_json::Value::Bool(v) => {
            if expected_type == TableDataType::Boolean {
                Ok(vec![v as u8])
            } else {
                Err(AppError::ValueTypeMismatch {
                    actual: JsonValueType::Bool,
                    expected: expected_type.into(),
                })
            }
        }
        serde_json::Value::Number(number) => match expected_type {
            TableDataType::TinyInt => {
                parse_and_validate_int!(number, constraints, TableConstraints::TinyInt, i8)
            }
            TableDataType::SmallInt => {
                parse_and_validate_int!(number, constraints, TableConstraints::SmallInt, i16)
            }
            TableDataType::Int => {
                parse_and_validate_int!(number, constraints, TableConstraints::Int, i32)
            }
            TableDataType::BigInt => {
                parse_and_validate_int!(number, constraints, TableConstraints::BigInt, i64)
            }
            TableDataType::Float => {
                parse_and_validate_float!(number, constraints, TableConstraints::Float, f32)
            }
            TableDataType::Double => {
                parse_and_validate_float!(number, constraints, TableConstraints::Double, f64)
            }
            _ => Err(AppError::ValueTypeMismatch {
                actual: JsonValueType::Number,
                expected: expected_type.into(),
            }),
        },
        serde_json::Value::String(v) => {
            if expected_type == TableDataType::Text {
                if let Some(TableConstraints::Text {
                    min_length,
                    max_length,
                }) = constraints
                {
                    let chars_count = v.chars().count();
                    if let Some(min) = min_length
                        && chars_count < *min
                    {
                        return Err(AppError::ConstraintViolation {
                            reason: format!(
                                "文字列の長さ {} は最小長 {} を下回っています",
                                chars_count, min
                            ),
                        });
                    }
                    if let Some(max) = max_length
                        && chars_count > *max
                    {
                        return Err(AppError::ConstraintViolation {
                            reason: format!(
                                "文字列の長さ {} は最大長 {} を上回っています",
                                chars_count, max
                            ),
                        });
                    }
                }
                Ok(v.into())
            } else if expected_type == TableDataType::Enum {
                if let Some(TableConstraints::Enum {
                    choices, mapping, ..
                }) = constraints
                {
                    if !choices.contains(&v) {
                        return Err(AppError::ConstraintViolation {
                            reason: format!(
                                "値 '{}' は許可された選択肢に含まれていません: {:?}",
                                v, choices
                            ),
                        });
                    }
                    if let Some(&id) = mapping.get(&v) {
                        Ok(id.to_be_bytes().to_vec())
                    } else {
                        // APIから与えられたchoicesには存在するが、マッピングに無い場合は内部エラー
                        Err(AppError::InvalidStoredValue {
                            reason: format!("Enum値 '{}' の内部マッピングが見つかりません", v),
                        })
                    }
                } else {
                    Err(AppError::ConstraintViolation {
                        reason: "Enum型には制約 (choices) が必須です".to_string(),
                    })
                }
            } else {
                Err(AppError::ValueTypeMismatch {
                    actual: JsonValueType::String,
                    expected: expected_type.into(),
                })
            }
        }
        serde_json::Value::Array(_) => Err(AppError::ValueTypeMismatch {
            actual: JsonValueType::Array,
            expected: expected_type.into(),
        }),
        serde_json::Value::Object(_) => Err(AppError::ValueTypeMismatch {
            actual: JsonValueType::Object,
            expected: expected_type.into(),
        }),
    }
}

/// バイト列を、レイヤーのデータ型に基づいて JSON 値へ復元する関数
pub fn restore_value(
    expected_type: TableDataType,
    constraints: Option<&TableConstraints>,
    value: &[u8],
) -> Result<serde_json::Value, AppError> {
    match expected_type {
        TableDataType::Text => {
            let text =
                String::from_utf8(value.to_vec()).map_err(|_| AppError::InvalidStoredValue {
                    reason: "text value is not valid UTF-8".to_string(),
                })?;
            Ok(serde_json::Value::String(text))
        }
        TableDataType::TinyInt => restore_int!(value, i8),
        TableDataType::SmallInt => restore_int!(value, i16),
        TableDataType::Int => restore_int!(value, i32),
        TableDataType::BigInt => restore_int!(value, i64),
        TableDataType::Float => restore_float!(value, f32),
        TableDataType::Double => restore_float!(value, f64),
        TableDataType::Boolean => {
            if value.len() != 1 {
                return Err(AppError::InvalidStoredValue {
                    reason: format!("boolean value must be 1 byte, got {}", value.len()),
                });
            }
            match value[0] {
                0 => Ok(serde_json::Value::Bool(false)),
                1 => Ok(serde_json::Value::Bool(true)),
                other => Err(AppError::InvalidStoredValue {
                    reason: format!("boolean value must be 0 or 1, got {}", other),
                }),
            }
        }
        TableDataType::Enum => {
            if value.len() != 2 {
                // 後方互換性: もし2バイトでない場合は古い文字列フォーマットとして解釈を試みる
                if let Ok(text) = String::from_utf8(value.to_vec()) {
                    return Ok(serde_json::Value::String(text));
                }
                return Err(AppError::InvalidStoredValue {
                    reason: format!(
                        "enum value must be 2 bytes (or valid legacy text), got {}",
                        value.len()
                    ),
                });
            }
            let bytes: [u8; 2] = value.try_into().unwrap();
            let id = u16::from_be_bytes(bytes);

            if let Some(TableConstraints::Enum { mapping, .. }) = constraints {
                // id から文字列を逆引きする
                for (k, &v) in mapping {
                    if v == id {
                        return Ok(serde_json::Value::String(k.clone()));
                    }
                }
                Err(AppError::InvalidStoredValue {
                    reason: format!("enum ID {} はマッピングに存在しません", id),
                })
            } else {
                Err(AppError::InvalidStoredValue {
                    reason: "Enum制約が設定されていないため値を復元できません".to_string(),
                })
            }
        }
        TableDataType::Presence => {
            if !value.is_empty() {
                return Err(AppError::InvalidStoredValue {
                    reason: format!("presence value must be 0 bytes, got {}", value.len()),
                });
            }
            Ok(serde_json::Value::Null)
        }
    }
}
