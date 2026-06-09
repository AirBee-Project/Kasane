use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header::LOCATION},
    response::{IntoResponse, Response},
};

use crate::{
    AppState, error::AppError, models::database::table::CreateTableRequest,
    services::database::table::create as table_create_service,
};

#[utoipa::path(
    post,
    path = "/databases/{db_name}/tables",
    request_body = CreateTableRequest,
    responses(
        (status = 201, description = "Table created"),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Table already exists")
    ),
    tag = "tables"
)]
pub async fn table_create(
    Path(db_name): Path<String>,
    State(app_state): State<AppState>,
    Json(request): Json<CreateTableRequest>,
) -> Result<Response, AppError> {
    let table_name = request.name.clone();
    table_create_service::create(&app_state, &db_name, &table_name, request).await?;
    Ok((
        StatusCode::CREATED,
        [(
            LOCATION,
            format!("/databases/{}/tables/{}", db_name, table_name),
        )],
    )
        .into_response())
}
