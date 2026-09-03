use super::TableDataType;
use crate::models::database::table::data_type::TableConstraints;
use serde::{Deserialize, Serialize};

/// テーブルの基本情報。一覧で返す軽量ビューで、件数は含まない。
#[derive(Debug, Deserialize, Serialize)]
pub struct TableSummary {
    pub name: String,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
    pub constraints: Option<TableConstraints>,
    pub description: Option<String>,
    /// データが時間IDを持つかどうか。
    pub is_temporal: bool,
}

/// 単一テーブルの詳細情報。保持する空間ID(FlexId)の総数 `count` を必ず含む。
#[derive(Debug, Deserialize, Serialize)]
pub struct TableInfoResponse {
    pub name: String,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
    pub count: u64,
    pub constraints: Option<TableConstraints>,
    pub description: Option<String>,
    /// データが時間IDを持つかどうか。
    pub is_temporal: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TableListResponse(pub Vec<TableSummary>);
