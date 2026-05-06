use std::convert::TryFrom;

use crate::{
    error::AppError,
    models::table::{TableDataType, data_type::JsonValueType},
};

///JSONの値を、テーブルのデータ型に基づいて解釈し、バイト列に変換する関数
/// 期待される値の型と実際の値の型が一致しない場合や、数値が範囲外の場合には、適切なエラーを返す
pub fn interpret_value(
    expected_type: TableDataType,
    value: serde_json::Value,
) -> Result<Vec<u8>, AppError> {
    match value {
        serde_json::Value::Null => Err(AppError::ValueTypeMismatch {
            actual: JsonValueType::Null,
            expected: expected_type.into(),
        }),
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
        serde_json::Value::Number(number) => {
            if expected_type == TableDataType::Int {
                match number.as_i64() {
                    Some(v) => match i32::try_from(v) {
                        Ok(v) => Ok(v.to_be_bytes().to_vec()),
                        Err(_) => Err(AppError::NumericValueOutOfRange {
                            actual: number.to_string(),
                            expected: "i32".to_string(),
                        }),
                    },
                    None => Err(AppError::NumericValueOutOfRange {
                        actual: number.to_string(),
                        expected: "i32".to_string(),
                    }),
                }
            } else if expected_type == TableDataType::Float {
                match number.as_f64() {
                    Some(v) => {
                        let value = v as f32;
                        if value.is_finite() {
                            Ok(value.to_be_bytes().to_vec())
                        } else {
                            Err(AppError::NumericValueOutOfRange {
                                actual: number.to_string(),
                                expected: "f32".to_string(),
                            })
                        }
                    }
                    None => Err(AppError::NumericValueOutOfRange {
                        actual: number.to_string(),
                        expected: "f32".to_string(),
                    }),
                }
            } else {
                Err(AppError::ValueTypeMismatch {
                    actual: JsonValueType::Number,
                    expected: expected_type.into(),
                })
            }
        }
        serde_json::Value::String(v) => {
            if expected_type == TableDataType::Text {
                Ok(v.into())
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

/// バイト列を、テーブルのデータ型に基づいて JSON 値へ復元する関数
pub fn restore_value(
    expected_type: TableDataType,
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
        TableDataType::Int => {
            if value.len() != 4 {
                return Err(AppError::InvalidStoredValue {
                    reason: format!("int value must be 4 bytes, got {}", value.len()),
                });
            }

            let bytes: [u8; 4] = value.try_into().map_err(|_| AppError::InvalidStoredValue {
                reason: "int value has invalid byte length".to_string(),
            })?;
            Ok(serde_json::Value::Number(serde_json::Number::from(
                i32::from_be_bytes(bytes),
            )))
        }
        TableDataType::Float => {
            if value.len() != 4 {
                return Err(AppError::InvalidStoredValue {
                    reason: format!("float value must be 4 bytes, got {}", value.len()),
                });
            }

            let bytes: [u8; 4] = value.try_into().map_err(|_| AppError::InvalidStoredValue {
                reason: "float value has invalid byte length".to_string(),
            })?;
            let number = f32::from_be_bytes(bytes);
            let json_number = serde_json::Number::from_f64(number as f64).ok_or_else(|| {
                AppError::InvalidStoredValue {
                    reason: "float value cannot be represented as JSON number".to_string(),
                }
            })?;
            Ok(serde_json::Value::Number(json_number))
        }
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
    }
}
