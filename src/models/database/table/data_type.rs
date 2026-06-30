use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[repr(u8)]
#[derive(Debug, Deserialize, Serialize, ToSchema, Clone, PartialEq, Eq, Hash, Copy)]
/// Table内の時空間IDに付与する値の型。
pub enum TableDataType {
    /// 文字列
    Text = 0,
    /// 32ビット符号付き整数
    Int = 1,
    /// 32ビット浮動小数点数
    Float = 2,
    /// 真偽値
    Boolean = 3,
    /// 選択式文字列
    Enum = 4,
    /// 空間IDの存在のみを示す（制約や値を持たない）
    Presence = 5,
    /// 8ビット符号付き整数 (MySQL TINYINT相当)
    TinyInt = 6,
    /// 16ビット符号付き整数 (MySQL SMALLINT相当)
    SmallInt = 7,
    /// 64ビット符号付き整数 (MySQL BIGINT相当)
    BigInt = 8,
    /// 64ビット浮動小数点数 (MySQL DOUBLE相当)
    Double = 9,
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
    /// `TinyInt` 型に対する制約。
    #[schema(example = json!({"type": "TinyInt", "min": -128, "max": 127}))]
    TinyInt {
        /// 最小値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<i8>,
        /// 最大値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<i8>,
    },
    /// `SmallInt` 型に対する制約。
    #[schema(example = json!({"type": "SmallInt", "min": -32768, "max": 32767}))]
    SmallInt {
        /// 最小値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<i16>,
        /// 最大値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<i16>,
    },
    /// `Int` 型に対する制約。
    #[schema(example = json!({"type": "Int", "min": 0, "max": 100}))]
    Int {
        /// 最小値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<i32>,
        /// 最大値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<i32>,
    },
    /// `BigInt` 型に対する制約。
    #[schema(example = json!({"type": "BigInt", "min": 0, "max": 100}))]
    BigInt {
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
        min: Option<f32>,
        /// 最大値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<f32>,
    },
    /// `Double` 型に対する制約。
    #[schema(example = json!({"type": "Double", "min": 0.0, "max": 100.0}))]
    Double {
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
            TableDataType::TinyInt
            | TableDataType::SmallInt
            | TableDataType::Int
            | TableDataType::BigInt
            | TableDataType::Float
            | TableDataType::Double => JsonValueType::Number,
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
                        "最小文字数 ({}) は最大文字数 ({}) 以下である必要があります",
                        min, max
                    ));
                }
            }
            TableConstraints::TinyInt { min, max } => {
                if let (Some(min), Some(max)) = (min, max)
                    && min > max
                {
                    return Err(format!(
                        "最小値 ({}) は最大値 ({}) 以下である必要があります",
                        min, max
                    ));
                }
            }
            TableConstraints::SmallInt { min, max } => {
                if let (Some(min), Some(max)) = (min, max)
                    && min > max
                {
                    return Err(format!(
                        "最小値 ({}) は最大値 ({}) 以下である必要があります",
                        min, max
                    ));
                }
            }
            TableConstraints::Int { min, max } => {
                if let (Some(min), Some(max)) = (min, max)
                    && min > max
                {
                    return Err(format!(
                        "最小値 ({}) は最大値 ({}) 以下である必要があります",
                        min, max
                    ));
                }
            }
            TableConstraints::BigInt { min, max } => {
                if let (Some(min), Some(max)) = (min, max)
                    && min > max
                {
                    return Err(format!(
                        "最小値 ({}) は最大値 ({}) 以下である必要があります",
                        min, max
                    ));
                }
            }
            TableConstraints::Float { min, max } => {
                if let (Some(min), Some(max)) = (min, max)
                    && min > max
                {
                    return Err(format!(
                        "最小値 ({}) は最大値 ({}) 以下である必要があります",
                        min, max
                    ));
                }
            }
            TableConstraints::Double { min, max } => {
                if let (Some(min), Some(max)) = (min, max)
                    && min > max
                {
                    return Err(format!(
                        "最小値 ({}) は最大値 ({}) 以下である必要があります",
                        min, max
                    ));
                }
            }
            TableConstraints::Enum { .. } => {}
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
