use axum::{
    Json,
    extract::{Path, State},
};

use crate::{
    AppState, error::AppError, models::table::TableInfoResponse,
    services::table::table_info as table_info_service,
};

#[utoipa::path(
    get,
    path = "/layers/{name}",
    params(
        ("name" = String, Path, description = "Table name")
    ),
    responses(
        (status = 200, description = "Table information", body = TableInfoResponse),
        (status = 404, description = "Table not found")
    ),
    tag = "layers"
)]
pub async fn table_info(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<TableInfoResponse>, AppError> {
    let table = table_info_service::table_info(&app_state, &name).await?;
    let res = TableInfoResponse {
        name: table.name,
        data_type: table.data_type,
        max_zoom_level: table.max_zoom_level,
    };
    Ok(Json(res))
}
