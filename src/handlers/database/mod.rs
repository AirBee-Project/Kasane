use crate::middleware::auth::AuthUser;
use crate::{
    AppState,
    error::{AppError, AuthError},
    models::database::{
        CopyDatabaseRequest, CreateDatabaseRequest, DatabaseInfoResponse, UpdateDatabaseRequest,
    },
};
use axum::Extension;
use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header::LOCATION},
    response::{IntoResponse, Response},
};

/// データベースの作成
///
/// 新しいデータベースを作成します。この操作はGlobal Admin権限が必要です。
#[utoipa::path(
    post,
    path = "/databases",
    request_body = CreateDatabaseRequest,
    responses(
        (status = 201, body = DatabaseInfoResponse)
    ),
    security(("bearer_auth" = ["global_admin"])),
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

/// データベース情報の取得
///
/// 指定したデータベースの詳細情報を取得します。対象データベースのRead以上の権限が必要です。
#[utoipa::path(
    get,
    path = "/databases/{name}",
    params(
        ("name" = String, Path, description = "データベース名", example = "example_database")
    ),
    responses(
        (status = 200, body = DatabaseInfoResponse)
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

/// データベース一覧の取得
///
/// ユーザー権限に応じて、アクセス可能なデータベースの一覧を取得します。
///
/// - **グローバル管理者**: システム内の全データベースが見えます。
/// - **一般ユーザー**: 自分が権限を持っているデータベースだけが見えます。
#[utoipa::path(
    get,
    path = "/databases",
    responses(
        (status = 200, body = Vec<DatabaseInfoResponse>)
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

/// データベースの削除
///
/// 指定したデータベースを削除します。この操作はGlobal Admin権限が必要です。
#[utoipa::path(
    delete,
    path = "/databases/{name}",
    params(
        ("name" = String, Path, description = "データベース名", example = "example_database")
    ),
    responses(
        (status = 204)
    ),
    security(("bearer_auth" = ["global_admin"])),
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

/// データベース名の変更
///
/// 指定したデータベースの名前を変更します。対象データベースのManage以上の権限が必要です。
#[utoipa::path(
    patch,
    path = "/databases/{name}",
    params(
        ("name" = String, Path, description = "データベース名", example = "example_database")
    ),
    request_body = UpdateDatabaseRequest,
    responses(
        (status = 200, description = "成功")
    ),
    security(("bearer_auth" = [])),
    tag = "databases"
)]
pub async fn database_rename(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(db_name): Path<String>,
    Json(request): Json<UpdateDatabaseRequest>,
) -> Result<StatusCode, AppError> {
    crate::middleware::auth::check_privilege(
        &app_state,
        &auth_user,
        &db_name,
        crate::models::users::UserRole::Manage,
    )
    .await?;
    crate::services::database::rename(&app_state, &db_name, &request.new_name).await?;
    Ok(StatusCode::OK)
}

/// データベースのコピー
///
/// 指定したデータベースをコピーします。コピー元データベースに対するRead権限が必要です。
#[utoipa::path(
    post,
    path = "/databases/{name}/copy",
    params(
        ("name" = String, Path, description = "コピー元データベース名", example = "src_db")
    ),
    request_body = CopyDatabaseRequest,
    responses(
        (status = 201, body = DatabaseInfoResponse)
    ),
    security(("bearer_auth" = [])),
    tag = "databases"
)]
pub async fn database_copy(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(db_name): Path<String>,
    Json(request): Json<CopyDatabaseRequest>,
) -> Result<Response, AppError> {
    crate::middleware::auth::check_privilege(
        &app_state,
        &auth_user,
        &db_name,
        crate::models::users::UserRole::Read,
    )
    .await?;

    if db_name == request.destination_name {
        return Err(AppError::Conflict(
            "Source and destination database names must be different".to_string(),
        ));
    }

    let user_id = if auth_user.user.is_global_admin {
        None
    } else {
        Some(crate::models::id::UserId(auth_user.user.id))
    };

    let res =
        crate::services::database::copy(&app_state, &db_name, &request.destination_name, user_id)
            .await?;

    Ok((
        StatusCode::CREATED,
        [(LOCATION, format!("/databases/{}", request.destination_name))],
        Json(res),
    )
        .into_response())
}

pub mod table;
