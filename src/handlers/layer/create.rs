use axum::{
    Json,
    extract::State,
    http::{StatusCode, header::LOCATION},
    response::{IntoResponse, Response},
};

use crate::{
    AppState, error::AppError, models::layer::CreateLayerRequest,
    services::layer::create as layer_create_service,
};

#[utoipa::path(
    post,
    path = "/layers",
    request_body = CreateLayerRequest,
    responses(
        (status = 201, description = "Layer created"),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Layer already exists")
    ),
    tag = "layers"
)]
pub async fn layer_create(
    State(app_state): State<AppState>,
    Json(request): Json<CreateLayerRequest>,
) -> Result<Response, AppError> {
    layer_create_service::create(
        &app_state,
        &request.name,
        request.data_type.clone(),
        request.max_zoom_level,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        [(LOCATION, format!("/layers/{}", request.name))],
    )
        .into_response())
}
