use axum::{Json, extract::State};

use crate::{
    AppState,
    auth::RequireRead,
    error::AppError,
    models::layer::{LayerInfoResponse, LayerListResponse},
    services::layer::list as layer_list_service,
};

#[utoipa::path(
    get,
    path = "/layers",
    responses(
        (status = 200, description = "List of all layers", body = LayerListResponse)
    ),
    tag = "layers"
)]
pub async fn layer_list(
    _auth: RequireRead,
    State(app_state): State<AppState>,
) -> Result<Json<LayerListResponse>, AppError> {
    let layers = layer_list_service::list(&app_state).await?;
    let response = LayerListResponse(
        layers
            .into_iter()
            .map(|l| LayerInfoResponse {
                name: l.name,
                data_type: l.data_type,
                max_zoom_level: l.max_zoom_level,
            })
            .collect(),
    );
    Ok(Json(response))
}
