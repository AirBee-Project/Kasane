use super::TableDataType;
use crate::models::database::table::data_type::TableConstraints;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, serde::Serialize, Deserialize)]
pub struct TableMetadata {
    pub id: crate::models::id::TableId,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
    pub constraints: Option<TableConstraints>,
    pub description: Option<String>,
    /// テーブルが時空間データであるか。マイグレーション互換性のためにデフォルトをtrueとし、既存テーブルは時間データとして扱う。
    #[serde(default = "crate::models::helpers::default_true")]
    pub is_temporal: bool,
}
