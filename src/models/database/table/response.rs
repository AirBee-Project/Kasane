use super::TableDataType;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// テーブルの基本情報。一覧で返す軽量ビューで、件数は含まない。
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TableSummary {
    #[schema(example = "example_table")]
    pub name: String,
    #[schema(example = TableDataType::Int)]
    pub data_type: TableDataType,
    #[schema(example = 25)]
    pub max_zoom_level: u8,
}

/// 単一テーブルの詳細情報。保持する空間ID(FlexId)の総数 `count` を必ず含む。
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TableInfoResponse {
    #[schema(example = "example_table")]
    pub name: String,
    #[schema(example = TableDataType::Int)]
    pub data_type: TableDataType,
    #[schema(example = 25)]
    pub max_zoom_level: u8,
    #[schema(example = 100)]
    pub count: u64,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TableListResponse(pub Vec<TableSummary>);
