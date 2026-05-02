use axum::{
    Json,
    extract::State,
    http::{StatusCode, header::LOCATION},
    response::{IntoResponse, Response},
};

use crate::{
    AppState,
    error::AppError,
    models::table::{CreateTableRequest, entity::TableMetadata},
    services::table::create as table_create_service,
};

pub async fn create(
    State(app_state): State<AppState>,
    Json(request): Json<CreateTableRequest>,
) -> Result<Response, AppError> {
    table_create_service::create(
        &app_state,
        &request.name,
        TableMetadata {
            r#type: request.r#type,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        [(LOCATION, format!("/tables/{}", request.name))],
    )
        .into_response())
}
