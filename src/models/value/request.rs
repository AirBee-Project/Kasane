use serde::Deserialize;
use utoipa::ToSchema;

use crate::models::query::Query;

#[derive(Debug, Deserialize, ToSchema)]
///時空間IDの範囲を[Query]で指定して値を取得する
pub struct GetValueRequest {
    pub name: String,
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
