use crate::middleware::auth::AuthUser;
use crate::{
    AppState,
    error::{AppError, AuthError},
    models::database::{CreateDatabaseRequest, DatabaseInfoResponse},
};
use axum::Extension;
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
    security(("bearer_auth" = [])),
    tag = "databases"
)]
pub async fn database_create(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(request): Json<CreateDatabaseRequest>,
) -> Result<Response, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AuthError::RequiresGlobalAdmin.into());
    }
    let res = crate::services::database::create(&app_state, request.name.as_str()).await?;
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
    security(("bearer_auth" = [])),
    tag = "databases"
)]
pub async fn database_info(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(db_name): Path<String>,
) -> Result<Json<DatabaseInfoResponse>, AppError> {
    crate::middleware::auth::check_privilege(
        &app_state,
        &auth_user,
        &db_name,
        crate::models::users::UserRole::Read,
    )
    .await?;
    let res = crate::services::database::info(&app_state, &db_name).await?;
    Ok(Json(res))
}

#[utoipa::path(
    get,
    path = "/databases",
    responses(
        (status = 200, description = "List databases", body = Vec<DatabaseInfoResponse>)
    ),
    security(("bearer_auth" = [])),
    tag = "databases"
)]
pub async fn database_list(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<DatabaseInfoResponse>>, AppError> {
    let res = crate::services::database::list(
        &app_state,
        auth_user.user.is_global_admin,
        crate::models::id::UserId(auth_user.user.id),
    )
    .await?;
    Ok(Json(res))
}

#[utoipa::path(
    delete,
    path = "/databases/{name}",
    responses(
        (status = 204, description = "Removed successfully")
    ),
    security(("bearer_auth" = [])),
    tag = "databases"
)]
pub async fn remove_database(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(db_name): Path<String>,
) -> Result<StatusCode, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AuthError::RequiresGlobalAdmin.into());
    }
    crate::services::database::remove(&app_state, db_name.as_str()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub mod table;
