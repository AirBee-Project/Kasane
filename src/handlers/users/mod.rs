use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    AppState,
    error::AppError,
    middleware::auth::AuthUser,
    models::users::{
        CreateUserRequest, PrivilegeInfoResponse, UpdatePasswordRequest, UpdatePrivilegeRequest,
        UserInfoResponse,
    },
    services::users as users_service,
};

#[utoipa::path(
    get,
    path = "/users",
    responses(
        (status = 200, description = "List users successfully", body = [UserInfoResponse]),
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
pub async fn list_users(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<UserInfoResponse>>, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AppError::Forbidden(
            "Requires GlobalAdmin privileges".to_string(),
        ));
    }
    let users = users_service::list_users(&app_state)?;
    Ok(Json(users))
}

#[utoipa::path(
    post,
    path = "/users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created successfully"),
        (status = 409, description = "User already exists")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
pub async fn create_user(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<StatusCode, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AppError::Forbidden(
            "Requires GlobalAdmin privileges".to_string(),
        ));
    }
    users_service::create_user(&app_state, payload).await?;
    Ok(StatusCode::CREATED)
}

#[utoipa::path(
    delete,
    path = "/users/{username}",
    params(
        ("username" = String, Path, description = "Username")
    ),
    responses(
        (status = 204, description = "User deleted successfully"),
        (status = 404, description = "User not found")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
pub async fn delete_user(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(username): Path<String>,
) -> Result<StatusCode, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AppError::Forbidden(
            "Requires GlobalAdmin privileges".to_string(),
        ));
    }
    users_service::delete_user(&app_state, &username).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/users/{username}/password",
    params(
        ("username" = String, Path, description = "Username")
    ),
    request_body = UpdatePasswordRequest,
    responses(
        (status = 204, description = "Password updated successfully"),
        (status = 404, description = "User not found")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
pub async fn update_password(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(username): Path<String>,
    Json(payload): Json<UpdatePasswordRequest>,
) -> Result<StatusCode, AppError> {
    if !auth_user.user.is_global_admin && auth_user.user.username != username {
        return Err(AppError::Forbidden(
            "You can only change your own password".to_string(),
        ));
    }
    users_service::update_password(&app_state, &username, payload).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/users/{username}/privileges",
    params(
        ("username" = String, Path, description = "Username")
    ),
    responses(
        (status = 200, description = "Get privileges successfully", body = [PrivilegeInfoResponse]),
        (status = 404, description = "User not found")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
pub async fn get_privileges(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(username): Path<String>,
) -> Result<Json<Vec<PrivilegeInfoResponse>>, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AppError::Forbidden(
            "Requires GlobalAdmin privileges".to_string(),
        ));
    }
    let privs = users_service::get_privileges(&app_state, &username)?;
    Ok(Json(privs))
}

#[utoipa::path(
    put,
    path = "/users/{username}/privileges/{db_name}",
    params(
        ("username" = String, Path, description = "Username"),
        ("db_name" = String, Path, description = "Database name")
    ),
    request_body = UpdatePrivilegeRequest,
    responses(
        (status = 204, description = "Privilege updated successfully"),
        (status = 404, description = "User or database not found")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
pub async fn set_privilege(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((username, db_name)): Path<(String, String)>,
    Json(payload): Json<UpdatePrivilegeRequest>,
) -> Result<StatusCode, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AppError::Forbidden(
            "Requires GlobalAdmin privileges".to_string(),
        ));
    }
    users_service::set_privilege(&app_state, &username, &db_name, payload).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/users/{username}/privileges/{db_name}",
    params(
        ("username" = String, Path, description = "Username"),
        ("db_name" = String, Path, description = "Database name")
    ),
    responses(
        (status = 204, description = "Privilege deleted successfully"),
        (status = 404, description = "User or database not found")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
pub async fn delete_privilege(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((username, db_name)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AppError::Forbidden(
            "Requires GlobalAdmin privileges".to_string(),
        ));
    }
    users_service::delete_privilege(&app_state, &username, &db_name).await?;
    Ok(StatusCode::NO_CONTENT)
}
