use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::TableDataType;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
/// テーブルの現在の状態（メタデータ）を表すドメインモデル
pub struct TableInfo {
    pub name: String,
    pub r#type: TableDataType,
}
