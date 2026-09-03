use super::TableDataType;
use crate::models::database::table::data_type::TableConstraints;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
/// 時空間IDと値が対応するTableを作成する
pub struct CreateTableRequest {
    pub name: String,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
    /// 値に対する制約。指定しない場合は制約なしとなる。
    pub constraints: Option<TableConstraints>,
    pub description: Option<String>,
    /// 値インデックス（値による絞り込み用の二次索引）を維持するかどうか。既定は `false`。
    ///
    /// 有効にすると格納する空間 ID 1 件につき索引キーが 1 つ増えるので、書き込みは
    /// 目に見えて重くなる。**作成後は変更できない。**
    #[serde(default)]
    pub value_index: bool,
    /// テーブルが時間IDを扱うかどうか。空間のみのデータの場合には`false`にしておいた方がパフォーマンスが向上する。
    // 正確にはパフォーマンスは将来的に向上する予定である。ユーザーに対してはこの言い方で良い。
    #[serde(default = "crate::models::helpers::default_true")]
    pub is_temporal: bool,
}

// 書き込みクロージャはやり直しで複数回呼ばれうるので、`Clone` が要る。
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum UpdateTableConstraints {
    Text {
        #[serde(skip_serializing_if = "Option::is_none")]
        min_length: Option<Option<usize>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_length: Option<Option<usize>>,
    },
    Int {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<Option<i64>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<Option<i64>>,
    },
    Enum {
        #[serde(skip_serializing_if = "Option::is_none")]
        choices: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        add_choices: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        remove_choices: Option<Vec<String>>,
    },
}
