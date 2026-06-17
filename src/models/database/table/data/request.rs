use crate::models::spatial_id::SpatialId;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
/// 空間IDの配列を指定して値を取得する
pub struct GetDataRequest {
    pub spatial_ids: Vec<SpatialId>,
    #[serde(default)]
    pub zoom_level_policy: ZoomLevelPolicy,
}

#[derive(Debug, Deserialize, ToSchema)]
/// 空間IDの配列を指定して値を挿入する
pub struct InsertDataRequest {
    pub value: serde_json::Value,
    pub spatial_ids: Vec<SpatialId>,
    #[serde(default)]
    pub zoom_level_policy: ZoomLevelPolicy,
}

#[derive(Debug, Deserialize, ToSchema)]
/// 空間IDの配列を指定して値を削除する
pub struct RemoveDataRequest {
    pub spatial_ids: Vec<SpatialId>,
    #[serde(default)]
    pub zoom_level_policy: ZoomLevelPolicy,
}

#[derive(Debug, Deserialize, ToSchema, Default, Clone, Copy, PartialEq, Eq)]
/// 各Tableのmax_zoom_levelよりも小さなIDが入力された場合の挙動を設定できる
pub enum ZoomLevelPolicy {
    #[default]
    ///エラーを返す
    Error,
    ///そのIDを無視する
    Ignore,
    ///そのIDを含むmax_zoom_levelのIDに正規化する
    Normalize,
}
