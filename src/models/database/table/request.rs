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
    #[schema(example = "テーブルの説明です")]
    pub description: Option<String>,
    /// 値インデックス（値による絞り込み用の二次索引）を維持するかどうか。既定は `false`。
    ///
    /// 有効にすると格納する空間 ID 1 件につき索引キーが 1 つ増えるので、書き込みは
    /// 目に見えて重くなる。**作成後は変更できない。**
    #[serde(default)]
    #[schema(example = false)]
    pub value_index: bool,
    /// テーブルが時空間データ（時間情報を持つ）であるかどうか。
    /// サーバーサイドでは false の場合、書き込み時にのみ時間データを拒否します。
    #[serde(default = "crate::models::helpers::default_true")]
    #[schema(example = true)]
    pub is_temporal: bool,
}

// 書き込みクロージャはやり直しで複数回呼ばれうるので、`Clone` が要る。
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum UpdateTableConstraints {
    #[schema(example = json!({"type": "Text", "min_length": 1, "max_length": 100}))]
    Text {
        #[serde(skip_serializing_if = "Option::is_none")]
        min_length: Option<Option<usize>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_length: Option<Option<usize>>,
    },
    #[schema(example = json!({"type": "Int", "min": 0, "max": 100}))]
    Int {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<Option<i64>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<Option<i64>>,
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
    #[serde(default, deserialize_with = "crate::models::helpers::double_option")]
    #[schema(value_type = Option<UpdateTableConstraints>, example = json!({"type": "Int", "min": 0, "max": 100}))]
    pub constraints: Option<Option<UpdateTableConstraints>>,

    #[serde(default, deserialize_with = "crate::models::helpers::double_option")]
    #[schema(value_type = Option<String>, example = "更新されたテーブルの説明文です")]
    pub description: Option<Option<String>>,
}

#[derive(Debug, serde::Deserialize, ToSchema)]
/// テーブルのコピーを行う
pub struct CopyTableRequest {
    #[schema(example = "example_database")]
    pub copy_db_name: Option<String>,
    #[schema(example = "example_table_copy")]
    pub copy_table_name: String,
}
