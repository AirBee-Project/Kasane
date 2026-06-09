use crate::AppState;
use crate::middleware::auth::AuthUser;
use crate::models::database::table::data::{GetDataRequest, GetDataResponse};
use crate::{error::AppError, services::database::table::data::get as data_get_service};
use axum::Extension;
use axum::{
    Json,
    extract::{Path, State},
};

#[utoipa::path(
    post,
    path = "/databases/{db_name}/tables/{table_name}/data/search",
    request_body = GetDataRequest,
    responses(
        (status = 200, description = "Data retrieved", body = GetDataResponse),
        (status = 404, description = "Table not found")
    ),
    tag = "data"
)]
pub async fn data_get(
    State(app_state): State<AppState>,
    Extension(_auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
    Json(payload): Json<crate::models::database::table::data::GetDataRequest>,
) -> Result<Json<crate::models::database::table::data::GetDataResponse>, AppError> {
    let result = data_get_service::get(
        &app_state,
        &db_name,
        &table_name,
        payload.query,
        &payload.zoom_level_policy,
    )
    .await?;
    Ok(Json(result))
}
