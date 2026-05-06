use serde::Deserialize;
use utoipa::ToSchema;

use super::query::Query;

#[derive(Debug, Deserialize, ToSchema)]
///時空間IDと値が対応するTableを作成する
pub struct CreateTableRequest {
    pub name: String,
    pub r#type: super::TableDataType,
    pub max_zoom_level: u8,
}

#[derive(Debug, Deserialize, ToSchema)]
///時空間IDの範囲を[Query]で指定して値を取得する
pub struct GetValueRequest {
    pub name: String,
    pub value: serde_json::Value,
    pub query: Query,
}

#[derive(Debug, Deserialize, ToSchema)]
///時空間IDの範囲を[Query]で指定して値を挿入する
pub struct InsertValueRequest {
    pub name: String,
    pub value: serde_json::Value,
    pub query: Query,
}

#[derive(Debug, Deserialize, ToSchema)]
///時空間IDの範囲を[Query]で指定して値を削除する
pub struct RemoveValueRequest {
    pub name: String,
    pub query: Query,
}
