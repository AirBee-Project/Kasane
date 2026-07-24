use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[repr(u8)]
#[derive(Debug, Deserialize, Serialize, ToSchema, Clone, PartialEq, Eq, Hash, Copy)]
/// Table内の時空間IDに付与する値の型。
pub enum TableDataType {
    /// 文字列
    Text = 0,
    /// 64ビット符号付き整数
    Int = 1,
    /// 64ビット浮動小数点数
    Float = 2,
    /// 真偽値
    Boolean = 3,
    /// 選択式文字列
    Enum = 4,
    /// 空間IDの存在のみを示す（制約や値を持たない）
    Presence = 5,
}

/// テーブルの値に対する制約。
///
/// `type` フィールドで制約の種類を指定する。指定できる種類は対象のデータ型に依存する。
/// - `Text`: 文字列の長さ制約（`min_length`, `max_length`）
/// - `Int` / `Float`: 数値の範囲制約（`min`, `max`）
/// - `Enum`: 許容される選択肢の制約（`choices`）
///
/// `Presence` および `Boolean` には制約を指定できない。
#[derive(Debug, Deserialize, Serialize, ToSchema, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum TableConstraints {
    /// `Text` 型に対する制約。
    #[schema(example = json!({"type": "Text", "min_length": 1, "max_length": 100}))]
    Text {
        /// 最小文字数
        #[serde(skip_serializing_if = "Option::is_none")]
        min_length: Option<usize>,
        /// 最大文字数
        #[serde(skip_serializing_if = "Option::is_none")]
        max_length: Option<usize>,
    },
    /// `Int` 型に対する制約。
    #[schema(example = json!({"type": "Int", "min": 0, "max": 100}))]
    Int {
        /// 最小値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<i64>,
        /// 最大値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<i64>,
    },
    /// `Float` 型に対する制約。
    #[schema(example = json!({"type": "Float", "min": 0.0, "max": 100.0}))]
    Float {
        /// 最小値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        /// 最大値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
    },
    /// `Enum` 型に対する制約。
    #[schema(example = json!({"type": "Enum", "choices": ["red", "blue", "green"]}))]
    Enum {
        /// 許容される文字列の配列
        choices: Vec<String>,
        /// 内部用: 文字列から整数IDへのマッピング
        #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
        mapping: std::collections::HashMap<String, u16>,
        /// 内部用: 次に割り当てる整数ID
        #[serde(default, skip_serializing_if = "is_zero")]
        next_id: u16,
    },
}

fn is_zero(num: &u16) -> bool {
    *num == 0
}

impl From<TableDataType> for JsonValueType {
    fn from(value: TableDataType) -> Self {
        match value {
            TableDataType::Text | TableDataType::Enum => JsonValueType::String,
            TableDataType::Int | TableDataType::Float => JsonValueType::Number,
            TableDataType::Boolean => JsonValueType::Bool,
            TableDataType::Presence => JsonValueType::Null,
        }
    }
}

impl TableConstraints {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            TableConstraints::Text {
                min_length,
                max_length,
            } => {
                if let (Some(min), Some(max)) = (min_length, max_length)
                    && min > max
                {
                    return Err(format!(
                        "min_length ({}) must be less than or equal to max_length ({})",
                        min, max
                    ));
                }
            }
            TableConstraints::Int { min, max } => {
                if let (Some(min), Some(max)) = (min, max)
                    && min > max
                {
                    return Err(format!(
                        "min ({}) must be less than or equal to max ({})",
                        min, max
                    ));
                }
            }
            TableConstraints::Float { min, max } => {
                if let (Some(min), Some(max)) = (min, max)
                    && min > max
                {
                    return Err(format!(
                        "min ({}) must be less than or equal to max ({})",
                        min, max
                    ));
                }
            }
            TableConstraints::Enum { choices, .. } => {
                for c in choices {
                    if c.is_empty() {
                        return Err("Enum choice cannot be empty".to_string());
                    }
                    let count = c.chars().count();
                    if count > 255 {
                        return Err(format!(
                            "Enum choice '{}' exceeds maximum length of 255 characters (actual: {})",
                            c, count
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsonValueType {
    String,
    Number,
    Bool,
    Array,
    Object,
    Null,
}
