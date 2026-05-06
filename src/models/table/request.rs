use serde::Deserialize;
use utoipa::ToSchema;

use super::TableDataType;

#[derive(Debug, Deserialize, ToSchema)]
///時空間IDと値が対応するTableを作成する
pub struct CreateTableRequest {
    #[schema(example = "my_table")]
    pub name: String,
    #[schema(example = TableDataType::Int)]
    pub data_type: TableDataType,
    #[schema(example = 25, maximum = 30)]
    pub max_zoom_level: u8,
}
