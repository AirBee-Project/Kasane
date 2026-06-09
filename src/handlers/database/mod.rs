use crate::{
    AppState,
    error::AppError,
    models::database::{CreateDatabaseRequest, DatabaseInfoResponse},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header::LOCATION},
    response::{IntoResponse, Response},
};

#[utoipa::path(
    post,
    path = "/databases",
    request_body = CreateDatabaseRequest,
    responses(
        (status = 201, description = "Created successfully", body = DatabaseInfoResponse)
    ),
    tag = "databases"
)]
pub async fn database_create(
    State(app_state): State<AppState>,
    Json(request): Json<CreateDatabaseRequest>,
) -> Result<Response, AppError> {
    let res = crate::services::database::create(&app_state, &request.name).await?;
    Ok((
        StatusCode::CREATED,
        [(LOCATION, format!("/databases/{}", request.name))],
        Json(res),
    )
        .into_response())
}

#[utoipa::path(
    get,
    path = "/databases/{name}",
    responses(
        (status = 200, description = "Get database info", body = DatabaseInfoResponse)
    ),
    tag = "databases"
)]
pub async fn database_info(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<DatabaseInfoResponse>, AppError> {
    let res = crate::services::database::info(&app_state, &name).await?;
    Ok(Json(res))
}

#[utoipa::path(
    get,
    path = "/databases",
    responses(
        (status = 200, description = "List databases", body = Vec<DatabaseInfoResponse>)
    ),
    tag = "databases"
)]
pub async fn database_list(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<DatabaseInfoResponse>>, AppError> {
    let res = crate::services::database::list(&app_state).await?;
    Ok(Json(res))
}

#[utoipa::path(
    delete,
    path = "/databases/{name}",
    responses(
        (status = 200, description = "Removed successfully")
    ),
    tag = "databases"
)]
pub async fn database_remove(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    crate::services::database::remove(&app_state, &name).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub mod table;
