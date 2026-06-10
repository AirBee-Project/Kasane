use crate::middleware::auth::AuthUser;
use axum::Extension;
use axum::{
    Json,
    extract::{Path, State},
};

use crate::{
    AppState, error::AppError, models::database::table::TableInfoResponse,
    services::database::table::info as table_info_service,
};

#[utoipa::path(
    get,
    path = "/databases/{db_name}/tables/{table_name}",
    responses(
        (status = 200, description = "Table info", body = TableInfoResponse),
        (status = 404, description = "Table not found")
    ),
    tag = "tables"
)]
pub async fn table_info(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((db_name, table_name)): Path<(String, String)>,
) -> Result<Json<TableInfoResponse>, AppError> {
    crate::middleware::auth::check_privilege(
        &app_state,
        &auth_user,
        &db_name,
        crate::models::users::UserRole::Manage,
    )
    .await?;

    let res = table_info_service::info(&app_state, &db_name, &table_name).await?;
    Ok(Json(res))
}
