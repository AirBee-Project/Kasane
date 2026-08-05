use crate::models::spatial_id::SpatialId;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
/// 空間IDの配列を指定して値を取得する
pub struct GetDataRequest {
    pub spatial_ids: Vec<SpatialId>,
}

#[derive(Debug, Deserialize, ToSchema, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum OutputFormat {
    SingleId,
    #[default]
    RangeId,
    FlexId,
}

#[derive(Debug, Deserialize, ToSchema, Default)]
pub struct GetDataQuery {
    #[serde(default)]
    pub format: OutputFormat,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, ToSchema)]
/// 空間IDの配列を指定して値を挿入する
pub struct InsertDataRequest {
    pub value: serde_json::Value,
    pub spatial_ids: Vec<SpatialId>,
}

#[derive(Debug, Deserialize, ToSchema)]
/// 空間IDの配列を指定して値を削除する
pub struct RemoveDataRequest {
    pub spatial_ids: Vec<SpatialId>,
}
