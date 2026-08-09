use crate::middleware::auth::AuthUser;
use crate::{
    AppState,
    error::AppError,
    models::database::{
        CopyDatabaseRequest, CreateDatabaseRequest, DatabaseInfoResponse, UpdateDatabaseRequest,
    },
    models::users::UserRole,
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
/// **必要な権限**: `global` / `manage`
///
/// 新しいデータベースを作成します。
#[utoipa::path(
    post,
    path = "/databases",
    request_body = CreateDatabaseRequest,
    responses(
        (status = 201, body = DatabaseInfoResponse)
    ),
    security(("bearer_auth" = [])),
    tag = "Databases"
)]
#[tracing::instrument(skip_all)]
pub async fn database_create(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(request): Json<CreateDatabaseRequest>,
) -> Result<Response, AppError> {
    crate::middleware::auth::check_global_role(&auth_user, UserRole::Manage)?;
    let res =
        crate::services::database::create(&app_state, request.name.as_str(), request.description)
            .await?;
    Ok((
        StatusCode::CREATED,
        [(LOCATION, format!("/databases/{}", request.name))],
        Json(res),
    )
        .into_response())
}

/// データベース情報の取得
///
/// **必要な権限**: `database` / `read`（配下テーブルの権限でも可）
///
/// 指定したデータベースの詳細情報を取得します。
#[utoipa::path(
    get,
    path = "/databases/{db_name}",
    params(
        ("db_name" = String, Path, description = "データベース名", example = "example_database")
    ),
    responses(
        (status = 200, body = DatabaseInfoResponse)
    ),
    security(("bearer_auth" = [])),
    tag = "Databases"
)]
#[tracing::instrument(skip_all)]
pub async fn database_info(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(db_name): Path<String>,
) -> Result<Json<DatabaseInfoResponse>, AppError> {
    crate::middleware::auth::check_database_visible(&app_state, &auth_user, &db_name)?;
    let res = crate::services::database::info(&app_state, &db_name).await?;
    Ok(Json(res))
}

/// データベース一覧の取得
///
/// **必要な権限**: なし（認証のみ・結果は権限で絞られる）
///
/// 呼び出したユーザーが Read 以上で到達できるデータベースだけを返します。
/// テーブル単位の権限しか持たない場合も、そのテーブルを含むデータベースは一覧に現れます。
#[utoipa::path(
    get,
    path = "/databases",
    responses(
        (status = 200, body = Vec<DatabaseInfoResponse>)
    ),
    security(("bearer_auth" = [])),
    tag = "Databases"
)]
#[tracing::instrument(skip_all)]
pub async fn database_list(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<DatabaseInfoResponse>>, AppError> {
    let res = crate::services::database::list(&app_state, &auth_user).await?;
    Ok(Json(res))
}

/// データベースの削除
///
/// **必要な権限**: `global` / `manage`
///
/// 指定したデータベースを削除します。
#[utoipa::path(
    delete,
    path = "/databases/{db_name}",
    params(
        ("db_name" = String, Path, description = "データベース名", example = "example_database")
    ),
    responses(
        (status = 204)
    ),
    security(("bearer_auth" = [])),
    tag = "Databases"
)]
#[tracing::instrument(skip_all)]
pub async fn remove_database(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(db_name): Path<String>,
) -> Result<StatusCode, AppError> {
    crate::middleware::auth::check_global_role(&auth_user, UserRole::Manage)?;
    crate::services::database::remove(&app_state, db_name.as_str()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// データベースの更新
///
/// **必要な権限**: `database` / `manage`
///
/// 指定したデータベースの名前や説明を変更します。
/// 権限はデータベース ID に紐づくため、改名しても権限は追従します。
#[utoipa::path(
    patch,
    path = "/databases/{db_name}",
    params(
        ("db_name" = String, Path, description = "データベース名", example = "example_database")
    ),
    request_body = UpdateDatabaseRequest,
    responses(
        (status = 200, description = "成功"),
        (status = 400, description = "リクエストが不正（パラメータエラー、または新しいデータベース名が不正）"),
        (status = 403, description = "権限不足（Manage権限がない場合）"),
        (status = 404, description = "変更元のデータベースが存在しない"),
        (status = 409, description = "変更後のデータベース名がすでに存在する")
    ),
    security(("bearer_auth" = [])),
    tag = "Databases"
)]
#[tracing::instrument(skip_all)]
pub async fn database_update(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(db_name): Path<String>,
    Json(request): Json<UpdateDatabaseRequest>,
) -> Result<StatusCode, AppError> {
    crate::middleware::auth::check_database(&app_state, &auth_user, &db_name, UserRole::Manage)?;
    crate::services::database::update(&app_state, &db_name, request.new_name, request.description)
        .await?;
    Ok(StatusCode::OK)
}

/// データベースのコピー
///
/// **必要な権限**: `global` / `manage`
///
/// 指定したデータベースをコピーします。
#[utoipa::path(
    post,
    path = "/databases/{db_name}/copy",
    params(
        ("db_name" = String, Path, description = "コピー元データベース名", example = "source_database")
    ),
    request_body = CopyDatabaseRequest,
    responses(
        (status = 201, body = DatabaseInfoResponse),
        (status = 400, description = "リクエストが不正（パラメータエラー、または不正なデータベース名）"),
        (status = 403, description = "権限不足（Global Admin権限がない場合）"),
        (status = 404, description = "コピー元データベースが存在しない"),
        (status = 409, description = "コピー先データベース、またはコピー元と同名のデータベースがすでに存在する")
    ),
    security(("bearer_auth" = [])),
    tag = "Databases"
)]
#[tracing::instrument(skip_all)]
pub async fn database_copy(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(db_name): Path<String>,
    Json(request): Json<CopyDatabaseRequest>,
) -> Result<Response, AppError> {
    crate::middleware::auth::check_global_role(&auth_user, UserRole::Manage)?;

    if db_name == request.copy_name {
        return Err(AppError::Conflict(
            "Source and destination database names must be different".to_string(),
        ));
    }

    let res = crate::services::database::copy(&app_state, &db_name, &request.copy_name).await?;

    Ok((
        StatusCode::CREATED,
        [(LOCATION, format!("/databases/{}", request.copy_name))],
        Json(res),
    )
        .into_response())
}

pub mod table;
