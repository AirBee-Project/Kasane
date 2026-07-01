use super::TableDataType;
use crate::models::database::table::data_type::TableConstraints;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
/// 時空間IDと値が対応するTableを作成する
pub struct CreateTableRequest {
    #[schema(example = "example_table")]
    pub name: String,
    #[schema(example = TableDataType::Int)]
    pub data_type: TableDataType,
    #[schema(example = 25, maximum = 30)]
    pub max_zoom_level: u8,
    /// 値に対する制約。指定しない場合は制約なしとなる。
    #[schema(example = json!({"type": "Int", "min": 0, "max": 100}))]
    pub constraints: Option<TableConstraints>,
}

fn default_validate_existing_data() -> bool {
    true
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum UpdateTableConstraints {
    #[schema(example = json!({"type": "Text", "min_length": 1, "max_length": 100}))]
    Text {
        #[serde(skip_serializing_if = "Option::is_none")]
        min_length: Option<Option<usize>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_length: Option<Option<usize>>,
    },
    #[schema(example = json!({"type": "TinyInt", "min": 0, "max": 100}))]
    TinyInt {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<Option<i8>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<Option<i8>>,
    },
    #[schema(example = json!({"type": "SmallInt", "min": 0, "max": 100}))]
    SmallInt {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<Option<i16>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<Option<i16>>,
    },
    #[schema(example = json!({"type": "Int", "min": 0, "max": 100}))]
    Int {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<Option<i32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<Option<i32>>,
    },
    #[schema(example = json!({"type": "BigInt", "min": 0, "max": 100}))]
    BigInt {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<Option<i64>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<Option<i64>>,
    },
    #[schema(example = json!({"type": "Float", "min": 0.0, "max": 100.0}))]
    Float {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<Option<f32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<Option<f32>>,
    },
    #[schema(example = json!({"type": "Double", "min": 0.0, "max": 100.0}))]
    Double {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<Option<f64>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<Option<f64>>,
    },
    #[schema(example = json!({"type": "Enum", "add_choices": ["yellow"], "remove_choices": ["red"]}))]
    Enum {
        #[serde(skip_serializing_if = "Option::is_none")]
        choices: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        add_choices: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        remove_choices: Option<Vec<String>>,
    },
}

#[derive(Debug, Deserialize, ToSchema)]
/// テーブルの名前変更または制約を更新する
pub struct UpdateTableRequest {
    #[schema(example = "example_table_renamed")]
    pub name: Option<String>,
    /// 更新後の値に対する制約。指定しない場合は制約を削除する。
    #[schema(example = json!({"type": "Int", "min": 0, "max": 100}))]
    pub constraints: Option<UpdateTableConstraints>,
    #[serde(default = "default_validate_existing_data")]
    #[schema(example = true, default = true)]
    pub validate_existing_data: bool,
}

#[derive(Debug, serde::Deserialize, ToSchema)]
/// テーブルのコピーを行う
pub struct CopyTableRequest {
    #[schema(example = "example_database")]
    pub copy_db_name: Option<String>,
    #[schema(example = "example_table_copy")]
    pub copy_table_name: String,
}
