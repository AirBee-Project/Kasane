use crate::middleware::auth::AuthUser;
use crate::models::users::UserRole;
use crate::{AppState, models::database::table::data::InsertDataRequest};
use crate::{error::AppError, services::database::table::data::insert as data_insert_service};
use axum::Extension;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

#[utoipa::path(
    put,
    path = "/databases/{db_name}/tables/{table_name}/data",
    request_body = InsertDataRequest,
    responses(
        (status = 200, description = "Data inserted"),
        (status = 404, description = "Table not found")
    ),
    security(("bearer_auth" = [])),
    tag = "data"
)]
pub async fn data_insert(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
    Json(payload): Json<InsertDataRequest>,
) -> Result<StatusCode, AppError> {
    crate::middleware::auth::check_privilege(&app_state, &auth_user, &db_name, UserRole::Write)
        .await?;

    data_insert_service::insert(
        &app_state,
        &db_name,
        &table_name,
        payload.query,
        payload.value,
        &payload.zoom_level_policy,
    )
    .await?;
    Ok(StatusCode::OK)
}
