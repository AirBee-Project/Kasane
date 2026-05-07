use serde::Deserialize;
use utoipa::ToSchema;
use super::LayerDataType;

#[derive(Debug, Deserialize, ToSchema)]
/// 時空間IDと値が対応するLayerを作成する
pub struct CreateLayerRequest {
    #[schema(example = "my_layer")]
    pub name: String,
    #[schema(example = LayerDataType::Int)]
    pub data_type: LayerDataType,
    #[schema(example = 25, maximum = 30)]
    pub max_zoom_level: u8,
}
