pub mod query;

use crate::models::table::query::Query;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, ToSchema)]
///Table内の時空間IDに付与する値の型を指定する
/// 型の名前はMySQLと同じ命名規則を採用
pub enum TableDataType {
    ///Rustの[String]に対応
    Text,
    ///Rustの[i32]に対応
    Int,
    ///Rustの[f32]に対応
    Float,
    ///Rustの[bool]に対応
    Boolean,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
///時空間IDと値が対応するTableを作成する
pub struct CreateTableRequest {
    pub name: String,
    pub r#type: TableDataType,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
///Tableを削除する
pub struct DropTableRequest {
    pub name: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
///時空間IDの範囲を[Query]で指定して取得する
pub struct SelectTableRequest {
    pub name: String,
    pub query: Query,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
///時空間IDの範囲を[Query]で指定して値を挿入する
pub struct InsertTableRequest<V> {
    pub name: String,
    pub value: V,
    pub query: Query,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
///時空間IDの範囲を[Query]で指定して値を削除する
pub struct DeleteTableRequest {
    pub name: String,
    pub query: Query,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct InfoTableResponse {
    pub name: String,
    pub r#type: TableDataType,
}
