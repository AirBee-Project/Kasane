use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash, Copy)]
/// Table内の時空間IDに付与する値の型。
pub enum TableDataType {
    /// 文字列
    Text = 0,
    /// 64ビット符号付き整数
    Int = 1,
    /// 真偽値
    Boolean = 3,
    /// 選択式文字列
    Enum = 4,
    /// 空間IDの存在のみを示す（制約や値を持たない）
    Presence = 5,
}

impl TableDataType {
    /// 格納バイト列の長さが、この型では常に一定か。
    ///
    /// 値インデックスのキーは値を可変長のまま連結するので、型の幅が一定なときだけ範囲
    /// スキャンの境界がそのまま正確な境界になる。可変長は `Text` だけ。
    pub fn has_fixed_width_value(self) -> bool {
        match self {
            // i64 = 8 / 真偽値 = 1 / Enum は選択肢 ID（u16）/ Presence は値なし。
            Self::Int | Self::Boolean | Self::Enum | Self::Presence => true,
            Self::Text => false,
        }
    }
}

/// テーブルの値に対する制約。
///
/// `type` フィールドで制約の種類を指定する。指定できる種類は対象のデータ型に依存する。
/// - `Text`: 文字列の長さ制約（`min_length`, `max_length`）
/// - `Int`: 数値の範囲制約（`min`, `max`）
/// - `Enum`: 許容される選択肢の制約（`choices`）
///
/// `Presence` および `Boolean` には制約を指定できない。
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum TableConstraints {
    /// `Text` 型に対する制約。
    Text {
        /// 最小文字数
        #[serde(skip_serializing_if = "Option::is_none")]
        min_length: Option<usize>,
        /// 最大文字数
        #[serde(skip_serializing_if = "Option::is_none")]
        max_length: Option<usize>,
    },
    /// `Int` 型に対する制約。
    Int {
        /// 最小値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<i64>,
        /// 最大値（境界値を含む）
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<i64>,
    },
    /// `Enum` 型に対する制約。
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
            TableDataType::Text | TableDataType::Enum => Self::String,
            TableDataType::Int => Self::Number,
            TableDataType::Boolean => Self::Bool,
            TableDataType::Presence => Self::Null,
        }
    }
}

impl TableConstraints {
    /// `Enum` の選択肢に未割り当ての ID を振る。
    ///
    /// 割り当て規則は**保存される値そのもの**なので、バックエンドごとに持たせるとストレージ間で
    /// 値の意味がずれる。
    pub fn with_enum_ids(
        data_type: TableDataType,
        constraints: Option<Self>,
    ) -> Result<Option<Self>, String> {
        let mut actual = constraints;
        if data_type != TableDataType::Enum {
            return Ok(actual);
        }
        match &mut actual {
            Some(Self::Enum {
                choices,
                mapping,
                next_id,
            }) => {
                for c in choices.iter() {
                    if !mapping.contains_key(c) {
                        if *next_id == u16::MAX {
                            return Err("Enum choices reached maximum limit (65535)".to_string());
                        }
                        if *next_id == 0 {
                            *next_id = 1;
                        }
                        mapping.insert(c.clone(), *next_id);
                        *next_id += 1;
                    }
                }
                Ok(actual)
            }
            _ => Err("Enum type requires 'choices' constraint".to_string()),
        }
    }

    /// 既存値を残したまま指定されたフィールドだけを差し替える。
    ///
    /// `Enum` は選択肢の増減を反映したうえで [`with_enum_ids`](Self::with_enum_ids) と同じ
    /// 規則で ID を振る。
    pub fn merged_with(
        data_type: TableDataType,
        current: Option<&Self>,
        update: super::UpdateTableConstraints,
    ) -> Result<Option<Self>, String> {
        use super::UpdateTableConstraints as Update;

        match (data_type, update) {
            (
                TableDataType::Text,
                Update::Text {
                    min_length,
                    max_length,
                },
            ) => {
                let (mut cur_min, mut cur_max) = match current {
                    Some(Self::Text {
                        min_length,
                        max_length,
                    }) => (*min_length, *max_length),
                    _ => (None, None),
                };
                if let Some(v) = min_length {
                    cur_min = v;
                }
                if let Some(v) = max_length {
                    cur_max = v;
                }
                Ok(Some(Self::Text {
                    min_length: cur_min,
                    max_length: cur_max,
                }))
            }
            (TableDataType::Int, Update::Int { min, max }) => {
                let (mut cur_min, mut cur_max) = match current {
                    Some(Self::Int { min, max }) => (*min, *max),
                    _ => (None, None),
                };
                if let Some(v) = min {
                    cur_min = v;
                }
                if let Some(v) = max {
                    cur_max = v;
                }
                Ok(Some(Self::Int {
                    min: cur_min,
                    max: cur_max,
                }))
            }
            (
                TableDataType::Enum,
                Update::Enum {
                    choices,
                    add_choices,
                    remove_choices,
                },
            ) => {
                let (mut cur_choices, mapping, next_id) = match current {
                    Some(Self::Enum {
                        choices,
                        mapping,
                        next_id,
                    }) => (choices.clone(), mapping.clone(), *next_id),
                    _ => (Vec::new(), std::collections::HashMap::new(), 1),
                };
                if let Some(new_choices) = choices {
                    cur_choices = new_choices;
                }
                if let Some(adds) = add_choices {
                    for add in adds {
                        if !cur_choices.contains(&add) {
                            cur_choices.push(add);
                        }
                    }
                }
                if let Some(removes) = remove_choices {
                    cur_choices.retain(|c| !removes.contains(c));
                }
                // ID の割り当ては新規作成時と同じ規則を通す。
                Self::with_enum_ids(
                    TableDataType::Enum,
                    Some(Self::Enum {
                        choices: cur_choices,
                        mapping,
                        next_id,
                    }),
                )
            }
            (TableDataType::Presence, _) => {
                Err("Presence type cannot have constraints".to_string())
            }
            (_, _) => Err("Constraint type does not match data type".to_string()),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Text {
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
            Self::Int { min, max } => {
                if let (Some(min), Some(max)) = (min, max)
                    && min > max
                {
                    return Err(format!(
                        "min ({}) must be less than or equal to max ({})",
                        min, max
                    ));
                }
            }
            Self::Enum { choices, .. } => {
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
