use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    AppState,
    error::{AppError, AuthError},
    middleware::auth::AuthUser,
    models::users::{
        CreateUserRequest, PrivilegeInfoResponse, UpdateAdminRequest, UpdatePasswordRequest,
        UpdatePrivilegeRequest, UserInfoResponse,
    },
    services::users as users_service,
};

/// ユーザー一覧の取得
///
/// ユーザーの一覧を取得します。この操作はGlobal Adminのみ実行可能です。
#[utoipa::path(
    get,
    path = "/users",
    responses(
        (status = 200, body = [UserInfoResponse]),
    ),
    security(("bearer_auth" = ["global_admin"])),
    tag = "Users"
)]
pub async fn list_users(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<UserInfoResponse>>, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AuthError::RequiresGlobalAdmin.into());
    }
    let users = users_service::list_users(&app_state)?;
    Ok(Json(users))
}

/// 新規ユーザー作成
///
/// 新しいユーザーを作成し、パスワードやGlobal Adminを設定します。この操作はGlobal Adminのみ実行可能です。
#[utoipa::path(
    post,
    path = "/users",
    request_body = CreateUserRequest,
    responses(
        (status = 201),
        (status = 409, description = "同名のユーザーが既に存在する")
    ),
    security(("bearer_auth" = ["global_admin"])),
    tag = "Users"
)]
pub async fn create_user(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<StatusCode, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AuthError::RequiresGlobalAdmin.into());
    }
    users_service::create_user(&app_state, payload).await?;
    Ok(StatusCode::CREATED)
}

/// ユーザーの削除
///
/// 指定したユーザーを削除します。この操作はGlobal Adminのみ実行可能です。
#[utoipa::path(
    delete,
    path = "/users/{username}",
    params(
        ("username" = String, Path, description = "ユーザー名")
    ),
    responses(
        (status = 204),
        (status = 404, description = "ユーザーが存在しない")
    ),
    security(("bearer_auth" = ["global_admin"])),
    tag = "Users"
)]
pub async fn delete_user(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(username): Path<String>,
) -> Result<StatusCode, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AuthError::RequiresGlobalAdmin.into());
    }
    users_service::delete_user(&app_state, &username).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// パスワードの更新
///
/// 指定したユーザーのパスワードを更新します。ユーザー本人が自分自身のパスワードを変更するか、Global Adminが他人のパスワードをリセットする場合に利用可能です。
#[utoipa::path(
    put,
    path = "/users/{username}/password",
    params(
        ("username" = String, Path, description = "ユーザー名")
    ),
    request_body = UpdatePasswordRequest,
    responses(
        (status = 204),
        (status = 404, description = "ユーザーが存在しない")
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
        return Err(AuthError::NotSelfOrAdmin.into());
    }

    if username == "root" && auth_user.user.username != "root" {
        return Err(AuthError::RootProtected.into());
    }
    users_service::update_password(&app_state, &username, payload).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Global Admin権限の変更
///
/// 指定したユーザーのGlobal Admin権限を付与または剥奪します。この操作はGlobal Adminのみ実行可能です。
#[utoipa::path(
    put,
    path = "/users/{username}/admin",
    params(
        ("username" = String, Path, description = "ユーザー名")
    ),
    request_body = UpdateAdminRequest,
    responses(
        (status = 204),
        (status = 403, description = "rootユーザーの権限は変更不可"),
        (status = 404, description = "ユーザーが存在しない")
    ),
    security(("bearer_auth" = ["global_admin"])),
    tag = "Users"
)]
pub async fn set_admin(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(username): Path<String>,
    Json(payload): Json<UpdateAdminRequest>,
) -> Result<StatusCode, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AuthError::RequiresGlobalAdmin.into());
    }
    users_service::set_admin(&app_state, &username, payload.is_global_admin).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// データベース権限の取得
///
/// 指定したユーザーが持つデータベースごとのアクセス権限（Read, Write, Manage）の一覧を取得します。この操作はグローバル管理者のみ実行可能です。
#[utoipa::path(
    get,
    path = "/users/{username}/privileges",
    params(
        ("username" = String, Path, description = "ユーザー名")
    ),
    responses(
        (status = 200, body = [PrivilegeInfoResponse]),
        (status = 404, description = "ユーザーが存在しない")
    ),
    security(("bearer_auth" = ["global_admin"])),
    tag = "Users"
)]
pub async fn get_privileges(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(username): Path<String>,
) -> Result<Json<Vec<PrivilegeInfoResponse>>, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AuthError::RequiresGlobalAdmin.into());
    }
    let privs = users_service::get_privileges(&app_state, &username)?;
    Ok(Json(privs))
}

/// データベース権限の設定
///
/// 指定したユーザーに対し、特定のデータベースへのアクセス権限（Read, Write, Manage）を設定します。この操作はグローバル管理者のみ実行可能です。
#[utoipa::path(
    put,
    path = "/users/{username}/privileges/{db_name}",
    params(
        ("username" = String, Path, description = "ユーザー名"),
        ("db_name" = String, Path, description = "データベース名")
    ),
    request_body = UpdatePrivilegeRequest,
    responses(
        (status = 204),
        (status = 404, description = "ユーザーまたはデータベースが存在しない")
    ),
    security(("bearer_auth" = ["global_admin"])),
    tag = "Users"
)]
pub async fn set_privilege(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((username, db_name)): Path<(String, String)>,
    Json(payload): Json<UpdatePrivilegeRequest>,
) -> Result<StatusCode, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AuthError::RequiresGlobalAdmin.into());
    }
    users_service::set_privilege(&app_state, &username, &db_name, payload).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// データベース権限の削除
///
/// 指定したユーザーから、特定のデータベースへのアクセス権限を削除します。この操作はGlobal Adminのみ実行可能です。
#[utoipa::path(
    delete,
    path = "/users/{username}/privileges/{db_name}",
    params(
        ("username" = String, Path, description = "ユーザー名"),
        ("db_name" = String, Path, description = "データベース名")
    ),
    responses(
        (status = 204),
        (status = 404, description = "ユーザーまたはデータベースが存在しない")
    ),
    security(("bearer_auth" = ["global_admin"])),
    tag = "Users"
)]
pub async fn delete_privilege(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((username, db_name)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AuthError::RequiresGlobalAdmin.into());
    }
    users_service::delete_privilege(&app_state, &username, &db_name).await?;
    Ok(StatusCode::NO_CONTENT)
}
