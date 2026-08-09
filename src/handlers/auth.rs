use crate::repositories::MetaRead;
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
    let span = tracing::Span::current();
    let token = tokio::task::spawn_blocking(move || -> Result<String, AppError> {
        let _guard = span.enter();
        let meta = app_state
            .db
            .read(|repo| repo.user_meta(&payload.username))?;

        let meta = match meta {
            Some(meta) => meta,
            None => {
                // ユーザーが存在しなくても実在時と同等の計算コストをかけ、
                // 応答時間差によるユーザー列挙を防ぐ。
                dummy_verify_password(&payload.password);
                return Err(AuthError::InvalidCredentials.into());
            }
        };

        if !verify_password(&payload.password, &meta.password_hash)? {
            return Err(AuthError::InvalidCredentials.into());
        }

        generate_jwt(&app_state, &payload.username)
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))??;

    Ok(Json(LoginResponse { token }))
}
