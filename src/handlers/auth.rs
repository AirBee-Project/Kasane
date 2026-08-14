use crate::repositories::{CatalogRepository, Storage};
use axum::{Json, extract::State};

use crate::{
    AppState,
    error::{AppError, AuthError},
    models::auth::{LoginRequest, LoginResponse},
    services::auth::{dummy_verify_password, generate_jwt, verify_password},
};

/// ユーザーログインとJWTの発行
///
/// **必要な権限**: なし（認証不要）
///
/// ユーザー名とパスワードを検証し、Bearerトークンを発行します。
#[utoipa::path(
    post,
    path = "/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, body = LoginResponse),
        (status = 401, description = "認証失敗（ユーザー名またはパスワードが不正）")
    ),
    security(),
    tag = "Auth"
)]
#[tracing::instrument(skip_all)]
pub async fn login(
    State(app_state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let username = payload.username.clone();
    let stored_hash = app_state
        .db
        .read(async move |repo| repo.user_record(&username).await)
        .await?
        .map(|meta| meta.password_hash);

    // argon2 の検証は CPU バウンドなのでブロッキングタスクへ逃がす。
    let password = payload.password.clone();
    let span = tracing::Span::current();
    let verified = tokio::task::spawn_blocking(move || -> Result<bool, AppError> {
        let _guard = span.enter();
        match &stored_hash {
            Some(hash) => verify_password(&password, hash),
            None => {
                // 実在時と同等の計算コストをかけ、応答時間差でのユーザー列挙を防ぐ。
                dummy_verify_password(&password);
                Ok(false)
            }
        }
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))??;

    if !verified {
        return Err(AuthError::InvalidCredentials.into());
    }

    let token = generate_jwt(&app_state, &payload.username).await?;

    Ok(Json(LoginResponse { token }))
}
