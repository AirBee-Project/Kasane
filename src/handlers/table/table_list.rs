use axum::{Json, extract::State};

use crate::{
    AppState,
    error::AppError,
    models::table::{TableInfoResponse, TableListResponse},
    services::table::table_list as table_list_service,
};

#[utoipa::path(
    get,
    path = "/tables",
    responses(
        (status = 200, description = "List of all tables", body = TableListResponse)
    ),
    tag = "tables"
)]
pub async fn table_list(
    State(app_state): State<AppState>,
) -> Result<Json<TableListResponse>, AppError> {
    let tables = table_list_service::table_list(&app_state).await?;
    let response = TableListResponse(
        tables
            .into_iter()
            .map(|t| TableInfoResponse {
                name: t.name,
                data_type: t.data_type,
                max_zoom_level: t.max_zoom_level,
            })
            .collect(),
    );
    Ok(Json(response))
}
