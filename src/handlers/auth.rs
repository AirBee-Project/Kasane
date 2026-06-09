use axum::{Json, extract::State};
use redb::ReadableDatabase;

use crate::{
    AppState,
    error::AppError,
    models::auth::{LoginRequest, LoginResponse},
    services::auth::{generate_jwt, verify_password},
};

#[utoipa::path(
    post,
    path = "/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Unauthorized")
    ),
    tag = "Auth"
)]
pub async fn login(
    State(app_state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let read_txn = app_state.redb.begin_read()?;
    let repo = crate::repositories::users::KasaneUsersRead::new(read_txn);

    let meta = repo
        .get_user_meta(&payload.username)?
        .ok_or_else(|| AppError::Unauthorized("Invalid username or password".to_string()))?;

    if !verify_password(&payload.password, &meta.password_hash)? {
        return Err(AppError::Unauthorized(
            "Invalid username or password".to_string(),
        ));
    }

    let token = generate_jwt(&payload.username)?;

    Ok(Json(LoginResponse { token }))
}
