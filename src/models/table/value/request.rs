use serde::Deserialize;
use utoipa::ToSchema;

use crate::models::query::Query;

#[derive(Debug, Deserialize, ToSchema)]
///空間IDの範囲を[Query]で指定して値を取得する
pub struct GetValueRequest {
    pub query: Query,
}

#[derive(Debug, Deserialize, ToSchema)]
///空間IDの範囲を[Query]で指定して値を挿入する
pub struct InsertValueRequest {
    pub query: Query,
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
///空間IDの範囲を[Query]で指定して値を削除する
pub struct RemoveValueRequest {
    pub query: Query,
}
