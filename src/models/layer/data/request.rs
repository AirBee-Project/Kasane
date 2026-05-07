use crate::models::query::Query;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
/// 空間IDの範囲を[Query]で指定して値を取得する
pub struct GetDataRequest {
    pub query: Query,
    #[serde(default)]
    pub zoom_level_policy: ZoomLevelPolicy,
}

#[derive(Debug, Deserialize, ToSchema)]
/// 空間IDの範囲を[Query]で指定して値を挿入する
pub struct InsertDataRequest {
    pub query: Query,
    pub value: serde_json::Value,
    #[serde(default)]
    pub zoom_level_policy: ZoomLevelPolicy,
}

#[derive(Debug, Deserialize, ToSchema)]
/// 空間IDの範囲を[Query]で指定して値を削除する
pub struct RemoveDataRequest {
    pub query: Query,
    #[serde(default)]
    pub zoom_level_policy: ZoomLevelPolicy,
}

#[derive(Debug, Deserialize, ToSchema, Default, Clone, Copy, PartialEq, Eq)]
/// 各Layerのmax_zoom_levelよりも小さなIDが入力された場合の挙動を設定できる
pub enum ZoomLevelPolicy {
    #[default]
    ///エラーを返す
    Error,
    ///そのIDを無視する
    Ignore,
    ///そのIDを含むmax_zoom_levelのIDに正規化する
    Normalize,
}
