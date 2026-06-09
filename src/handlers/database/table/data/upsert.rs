use crate::middleware::auth::AuthUser;
use crate::models::users::UserRole;
use crate::{AppState, models::database::table::data::InsertDataRequest};
use crate::{error::AppError, services::database::table::data::upsert as data_upsert_service};
use axum::Extension;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

#[utoipa::path(
    patch,
    path = "/databases/{db_name}/tables/{table_name}/data",
    request_body = InsertDataRequest,
    responses(
        (status = 204, description = "Data upserted"),
        (status = 404, description = "Table not found")
    ),
    tag = "tables"
)]
pub async fn data_upsert(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
    Json(payload): Json<InsertDataRequest>,
) -> Result<StatusCode, AppError> {
    crate::middleware::auth::check_privilege(&app_state, &auth_user, &db_name, UserRole::Write)
        .await?;

    data_upsert_service::upsert(
        &app_state,
        &db_name,
        &table_name,
        payload.query,
        payload.value,
        &payload.zoom_level_policy,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
