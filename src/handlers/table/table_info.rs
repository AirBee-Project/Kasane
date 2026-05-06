use axum::{
    Json,
    extract::{Path, State},
};

use crate::{
    AppState, error::AppError, models::table::InfoTableResponse,
    services::table::table_info as table_info_service,
};

#[utoipa::path(
    get,
    path = "/tables/{name}",
    params(
        ("name" = String, Path, description = "Table name")
    ),
    responses(
        (status = 200, description = "Table information", body = InfoTableResponse),
        (status = 404, description = "Table not found")
    ),
    tag = "tables"
)]
pub async fn info(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<InfoTableResponse>, AppError> {
    let res = table_info_service::table_info(&app_state, &name).await?;
    Ok(Json(res))
}
