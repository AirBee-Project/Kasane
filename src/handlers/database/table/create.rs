use crate::middleware::auth::AuthUser;
use crate::models::users::UserRole;
use axum::Extension;
use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header::LOCATION},
    response::{IntoResponse, Response},
};

use crate::{
    AppState, error::AppError, models::database::table::CreateTableRequest,
    services::database::table::create as table_create_service,
};

/// テーブルの作成
///
/// **必要な権限**: `database` / `manage`
///
/// 指定したデータベース内に新しいテーブルを作成します。
/// 新規作成はデータベース全体への変更なので、テーブル単位の権限では実行できません。
#[utoipa::path(
    post,
    path = "/databases/{db_name}/tables",
    params(
        ("db_name" = String, Path, description = "データベース名", example = "example_database")
    ),
    request_body = CreateTableRequest,
    responses(
        (status = 201),
        (status = 400, description = "リクエストが不正（パラメータエラーなど）"),
        (status = 409, description = "同名のテーブルが既に存在する")
    ),
    security(("bearer_auth" = [])),
    tag = "Tables"
)]
#[tracing::instrument(skip_all, fields(db_name = %db_name))]
pub async fn table_create(
    Path(db_name): Path<String>,
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(request): Json<CreateTableRequest>,
) -> Result<Response, AppError> {
    // 新しいテーブルの作成はデータベース全体への変更なので、データベースレベルの
    // Manage を要求する。テーブルスコープのルールは「既にあるテーブルの管理」しか許さない
    // （まだ存在しないテーブル名へのルールで作成させると、スコープの封じ込めが破れる）。
    crate::middleware::auth::check_database(&app_state, &auth_user, &db_name, UserRole::Manage)?;

    let table_name = request.name.clone();
    table_create_service::create(&app_state, &db_name, &table_name, request).await?;
    Ok((
        StatusCode::CREATED,
        [(
            LOCATION,
            format!("/databases/{}/tables/{}", db_name, table_name),
        )],
    )
        .into_response())
}
