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
/// 指定したデータベース内に新しいテーブルを作成します。この操作はデータベースのWrite以上の権限が必要です。
#[utoipa::path(
    post,
    path = "/databases/{db_name}/tables",
    request_body = CreateTableRequest,
    responses(
        (status = 201),
        (status = 400, description = "リクエストが不正（パラメータエラーなど）"),
        (status = 409, description = "同名のテーブルが既に存在する")
    ),
    security(("bearer_auth" = [])),
    tag = "tables"
)]
pub async fn table_create(
    Path(db_name): Path<String>,
    State(app_state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(request): Json<CreateTableRequest>,
) -> Result<Response, AppError> {
    crate::middleware::auth::check_privilege(&app_state, &auth_user, &db_name, UserRole::Manage)
        .await?;

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
