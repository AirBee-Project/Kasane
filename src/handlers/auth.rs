use axum::{Json, extract::State};

use crate::{
    AppState,
    error::{AppError, AuthError},
    models::auth::{LoginRequest, LoginResponse},
    services::auth::{dummy_verify_password, generate_jwt, verify_password},
};

/// ユーザーログインとJWTの発行
///
/// ユーザー名とパスワードを検証し、Bearerトークンを発行します。
#[utoipa::path(
    post,
    path = "/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "ログイン成功", body = LoginResponse),
        (status = 401, description = "認証失敗")
    ),
    tag = "Auth"
)]
pub async fn login(
    State(app_state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let read_txn = app_state.db.env.read_txn()?;
    let repo = crate::repositories::users::KasaneUsersRead::new(read_txn, &app_state.db);

    let meta = match repo.get_user_meta(&payload.username)? {
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

    drop(repo);

    let token = generate_jwt(&app_state, &payload.username)?;

    Ok(Json(LoginResponse { token }))
}
