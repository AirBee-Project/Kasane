use axum::{
    Json,
    extract::{Path, State},
};

use crate::{
    AppState, error::AppError, models::layer::LayerInfoResponse,
    services::layer::info as layer_info_service,
};

#[utoipa::path(
    get,
    path = "/layers/{name}",
    params(
        ("name" = String, Path, description = "Layer name")
    ),
    responses(
        (status = 200, description = "Layer information", body = LayerInfoResponse),
        (status = 404, description = "Layer not found")
    ),
    tag = "layers"
)]
pub async fn layer_info(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<LayerInfoResponse>, AppError> {
    let layer = layer_info_service::info(&app_state, &name).await?;
    let res = LayerInfoResponse {
        name: layer.name,
        data_type: layer.data_type,
        max_zoom_level: layer.max_zoom_level,
    };
    Ok(Json(res))
}
