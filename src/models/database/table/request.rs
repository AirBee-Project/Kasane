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
