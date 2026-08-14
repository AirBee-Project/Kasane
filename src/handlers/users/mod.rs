use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};

use crate::{
    AppState,
    error::AppError,
    middleware::auth::{AuthUser, check_global_admin, check_self_or_admin},
    models::users::{
        CreateUserRequest, ListUsersQuery, PrivilegeRule, PrivilegeTarget, PrivilegesResponse,
        SetDataPrivilegeRequest, SetGlobalPrivilegeRequest, UpdatePasswordRequest,
        UserInfoResponse, UserListResponse,
    },
    services::users as users_service,
};

/// ユーザー一覧の取得
///
/// **必要な権限**: `global` / `admin`
///
/// ユーザーの一覧を利用者名の辞書順で取得します。
#[utoipa::path(
    get,
    path = "/users",
    params(ListUsersQuery),
    responses(
        (status = 200, body = UserListResponse),
        (status = 401, description = "Unauthorized")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
#[tracing::instrument(skip_all)]
pub async fn list_users(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Query(query): Query<ListUsersQuery>,
) -> Result<Json<UserListResponse>, AppError> {
    check_global_admin(&auth_user)?;
    let users = users_service::list_users(&app_state, &query).await?;
    Ok(Json(users))
}

/// 新規ユーザー作成
///
/// **必要な権限**: `global` / `admin`
///
/// 新しいユーザーを作成します。`privileges` を指定すると作成と同時に権限を付与できます。
#[utoipa::path(
    post,
    path = "/users",
    request_body = CreateUserRequest,
    responses(
        (status = 201),
        (status = 400, description = "権限ルールが不正（global 以外での admin 指定など）"),
        (status = 404, description = "権限ルールが参照するデータベースまたはテーブルが存在しない"),
        (status = 409, description = "同名のユーザーが既に存在する")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
#[tracing::instrument(skip_all)]
pub async fn create_user(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<StatusCode, AppError> {
    check_global_admin(&auth_user)?;
    users_service::create_user(&app_state, payload).await?;
    Ok(StatusCode::CREATED)
}

/// ユーザーの削除
///
/// **必要な権限**: `global` / `admin`
///
/// 指定したユーザーを削除します。
#[utoipa::path(
    delete,
    path = "/users/{username}",
    params(
        ("username" = String, Path, description = "ユーザー名", example = "example_user")
    ),
    responses(
        (status = 204),
        (status = 403, description = "rootユーザーは削除不可"),
        (status = 404, description = "ユーザーが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
#[tracing::instrument(skip_all)]
pub async fn delete_user(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(username): Path<String>,
) -> Result<StatusCode, AppError> {
    check_global_admin(&auth_user)?;
    users_service::delete_user(&app_state, &username).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// ユーザー情報の取得
///
/// **必要な権限**: 本人 または `global` / `admin`
///
/// 指定したユーザーの情報を取得します。
#[utoipa::path(
    get,
    path = "/users/{username}",
    params(
        ("username" = String, Path, description = "ユーザー名", example = "example_user")
    ),
    responses(
        (status = 200, body = UserInfoResponse),
        (status = 404, description = "ユーザーが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
#[tracing::instrument(skip_all)]
pub async fn get_user(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(username): Path<String>,
) -> Result<Json<UserInfoResponse>, AppError> {
    check_self_or_admin(&auth_user, &username)?;
    let user = users_service::get_user(&app_state, &username).await?;
    Ok(Json(user))
}

/// パスワードの変更
///
/// **必要な権限**: 本人 または `global` / `admin`
///
/// 指定したユーザーのパスワードを変更します。変更すると発行済みトークンはすべて失効します。
#[utoipa::path(
    put,
    path = "/users/{username}/password",
    params(
        ("username" = String, Path, description = "ユーザー名", example = "example_user")
    ),
    request_body = UpdatePasswordRequest,
    responses(
        (status = 204),
        (status = 404, description = "ユーザーが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
#[tracing::instrument(skip_all)]
pub async fn update_password(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(username): Path<String>,
    Json(payload): Json<UpdatePasswordRequest>,
) -> Result<StatusCode, AppError> {
    check_self_or_admin(&auth_user, &username)?;

    users_service::update_password(&app_state, &username, payload).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 権限一覧の取得
///
/// **必要な権限**: 本人 または `global` / `admin`
///
/// 指定したユーザーが持つ権限ルールの一覧を取得します。
///
/// 既に削除されたデータベース・テーブルを指すルールは名前へ解決できないため一覧に現れません
/// （そうしたルールは認可判定でも一致しないため、表示と実効権限が食い違うことはありません）。
#[utoipa::path(
    get,
    path = "/users/{username}/privileges",
    params(
        ("username" = String, Path, description = "ユーザー名", example = "example_user")
    ),
    responses(
        (status = 200, body = PrivilegesResponse),
        (status = 404, description = "ユーザーが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
#[tracing::instrument(skip_all)]
pub async fn get_privileges(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(username): Path<String>,
) -> Result<Json<PrivilegesResponse>, AppError> {
    check_self_or_admin(&auth_user, &username)?;
    let privileges = users_service::get_privileges(&app_state, &username).await?;
    Ok(Json(PrivilegesResponse { privileges }))
}

/// サーバー全体に対する権限の設定
///
/// **必要な権限**: `global` / `admin`
///
/// 既に設定されていればロールを置き換えます。`admin` を指定できるのはこのスコープだけです。
#[utoipa::path(
    put,
    path = "/users/{username}/privileges/global",
    params(
        ("username" = String, Path, description = "ユーザー名", example = "example_user")
    ),
    request_body = SetGlobalPrivilegeRequest,
    responses(
        (status = 204),
        (status = 400, description = "保持できる権限数の上限を超えた"),
        (status = 403, description = "rootユーザーの権限は変更不可"),
        (status = 404, description = "ユーザーが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
#[tracing::instrument(skip_all)]
pub async fn set_global_privilege(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(username): Path<String>,
    Json(payload): Json<SetGlobalPrivilegeRequest>,
) -> Result<StatusCode, AppError> {
    check_global_admin(&auth_user)?;
    users_service::grant_privilege(
        &app_state,
        &username,
        PrivilegeRule::Global { role: payload.role },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// サーバー全体に対する権限の剥奪
///
/// **必要な権限**: `global` / `admin`
///
/// ロールを問わず、このスコープの権限を取り除きます。
#[utoipa::path(
    delete,
    path = "/users/{username}/privileges/global",
    params(
        ("username" = String, Path, description = "ユーザー名", example = "example_user")
    ),
    responses(
        (status = 204),
        (status = 403, description = "rootユーザーの権限は変更不可"),
        (status = 404, description = "ユーザーが存在しない、またはこのスコープの権限を持っていない")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
#[tracing::instrument(skip_all)]
pub async fn delete_global_privilege(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(username): Path<String>,
) -> Result<StatusCode, AppError> {
    check_global_admin(&auth_user)?;
    users_service::revoke_privilege(&app_state, &username, PrivilegeTarget::Global).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// データベースに対する権限の設定
///
/// **必要な権限**: `global` / `admin`
///
/// 既に設定されていればロールを置き換えます。配下のテーブルすべてに効きます。
/// 権限はデータベース ID に紐づくため、改名しても追従します。
#[utoipa::path(
    put,
    path = "/users/{username}/privileges/databases/{db_name}",
    params(
        ("username" = String, Path, description = "ユーザー名", example = "example_user"),
        ("db_name" = String, Path, description = "データベース名", example = "example_database")
    ),
    request_body = SetDataPrivilegeRequest,
    responses(
        (status = 204),
        (status = 400, description = "保持できる権限数の上限を超えた"),
        (status = 403, description = "rootユーザーの権限は変更不可"),
        (status = 404, description = "ユーザーまたはデータベースが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
#[tracing::instrument(skip_all)]
pub async fn set_database_privilege(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((username, db_name)): Path<(String, String)>,
    Json(payload): Json<SetDataPrivilegeRequest>,
) -> Result<StatusCode, AppError> {
    check_global_admin(&auth_user)?;
    users_service::grant_privilege(
        &app_state,
        &username,
        PrivilegeRule::Database {
            db_name,
            role: payload.role,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// データベースに対する権限の剥奪
///
/// **必要な権限**: `global` / `admin`
///
/// ロールを問わず、このデータベースに対する権限を取り除きます。
#[utoipa::path(
    delete,
    path = "/users/{username}/privileges/databases/{db_name}",
    params(
        ("username" = String, Path, description = "ユーザー名", example = "example_user"),
        ("db_name" = String, Path, description = "データベース名", example = "example_database")
    ),
    responses(
        (status = 204),
        (status = 403, description = "rootユーザーの権限は変更不可"),
        (status = 404, description = "ユーザー・データベースが存在しない、または権限を持っていない")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
#[tracing::instrument(skip_all)]
pub async fn delete_database_privilege(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((username, db_name)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    check_global_admin(&auth_user)?;
    users_service::revoke_privilege(&app_state, &username, PrivilegeTarget::Database { db_name })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// テーブルに対する権限の設定
///
/// **必要な権限**: `global` / `admin`
///
/// 既に設定されていればロールを置き換えます。
/// 権限はテーブル ID に紐づくため、改名しても追従します。
#[utoipa::path(
    put,
    path = "/users/{username}/privileges/databases/{db_name}/tables/{table_name}",
    params(
        ("username" = String, Path, description = "ユーザー名", example = "example_user"),
        ("db_name" = String, Path, description = "データベース名", example = "example_database"),
        ("table_name" = String, Path, description = "テーブル名", example = "example_table")
    ),
    request_body = SetDataPrivilegeRequest,
    responses(
        (status = 204),
        (status = 400, description = "保持できる権限数の上限を超えた"),
        (status = 403, description = "rootユーザーの権限は変更不可"),
        (status = 404, description = "ユーザー・データベース・テーブルのいずれかが存在しない")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
#[tracing::instrument(skip_all)]
pub async fn set_table_privilege(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((username, db_name, table_name)): Path<(String, String, String)>,
    Json(payload): Json<SetDataPrivilegeRequest>,
) -> Result<StatusCode, AppError> {
    check_global_admin(&auth_user)?;
    users_service::grant_privilege(
        &app_state,
        &username,
        PrivilegeRule::Table {
            db_name,
            table_name,
            role: payload.role,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// テーブルに対する権限の剥奪
///
/// **必要な権限**: `global` / `admin`
///
/// ロールを問わず、このテーブルに対する権限を取り除きます。
#[utoipa::path(
    delete,
    path = "/users/{username}/privileges/databases/{db_name}/tables/{table_name}",
    params(
        ("username" = String, Path, description = "ユーザー名", example = "example_user"),
        ("db_name" = String, Path, description = "データベース名", example = "example_database"),
        ("table_name" = String, Path, description = "テーブル名", example = "example_table")
    ),
    responses(
        (status = 204),
        (status = 403, description = "rootユーザーの権限は変更不可"),
        (status = 404, description = "対象が存在しない、または権限を持っていない")
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
#[tracing::instrument(skip_all)]
pub async fn delete_table_privilege(
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path((username, db_name, table_name)): Path<(String, String, String)>,
) -> Result<StatusCode, AppError> {
    check_global_admin(&auth_user)?;
    users_service::revoke_privilege(
        &app_state,
        &username,
        PrivilegeTarget::Table {
            db_name,
            table_name,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
