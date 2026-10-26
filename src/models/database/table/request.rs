use super::TableDataType;
use crate::models::database::table::data_type::TableConstraints;

#[derive(Debug)]
pub struct CreateTableRequest {
    pub name: String,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
    /// 値に対する制約。指定しない場合は制約なしとなる。
    pub constraints: Option<TableConstraints>,
    pub description: Option<String>,
    /// テーブルが時間IDを扱うかどうか。
    pub is_temporal: bool,
}
