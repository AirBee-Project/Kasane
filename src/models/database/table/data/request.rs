use crate::models::spatial_id::SpatialId;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
/// 空間IDの配列を指定して値を取得する
pub struct GetDataRequest {
    pub spatial_ids: Vec<SpatialId>,
    #[serde(default)]
    pub zoom_level_policy: ZoomLevelPolicy,
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum OutputFormat {
    SingleId,
    #[default]
    RangeId,
    FlexId,
}

#[derive(Debug, Deserialize, Default)]
pub struct GetDataQuery {
    #[serde(default)]
    pub format: OutputFormat,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
/// 空間IDの配列を指定して値を挿入する
pub struct InsertDataRequest {
    pub value: serde_json::Value,
    pub spatial_ids: Vec<SpatialId>,
    #[serde(default)]
    pub zoom_level_policy: ZoomLevelPolicy,
}

#[derive(Debug, Deserialize)]
/// 空間IDの配列を指定して値を削除する
pub struct RemoveDataRequest {
    pub spatial_ids: Vec<SpatialId>,
    #[serde(default)]
    pub zoom_level_policy: ZoomLevelPolicy,
}

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
/// テーブルの max_zoom_level よりも大きなズームレベル値の空間IDが入力された場合の処理方針
pub enum ZoomLevelPolicy {
    #[default]
    /// テーブルの max_zoom_level を超える空間IDが入力された場合に、エラー（400 Bad Request）を返す
    Error,
    /// テーブルの max_zoom_level を超える空間IDを処理対象から除外し、無視して続行する
    Ignore,
    /// テーブルの max_zoom_level を超える空間IDを、その範囲を内包する max_zoom_level の親空間IDに自動的に正規化して処理する
    Normalize,
}
