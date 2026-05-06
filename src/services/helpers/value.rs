use std::convert::TryFrom;

use crate::{
    error::AppError,
    models::table::{TableDataType, data_type::JsonValueType},
};

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
