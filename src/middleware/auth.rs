use axum::{Extension, extract::Request, http::header, middleware::Next, response::Response};

use crate::models::users::{User, UserRole};
use crate::{
    AppState,
    error::{AppError, AuthError},
    services::auth::verify_jwt,
};

#[derive(Clone)]
pub struct AuthUser {
    pub user: User,
}

pub async fn require_auth(
    axum::extract::State(app_state): axum::extract::State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = req.headers().get(header::AUTHORIZATION);

    let auth_header = match auth_header {
        Some(header) => header.to_str().unwrap_or(""),
        None => {
            return Err(AuthError::MissingToken.into());
        }
    };

    if !auth_header.starts_with("Bearer ") {
        return Err(AuthError::MalformedHeader.into());
    }

    let token = &auth_header[7..];
    let claims = verify_jwt(token)?;

    let user = app_state
        .db
        .read_users(|repo| repo.get_user(&claims.sub))?
        .ok_or(AppError::Auth(AuthError::TokenRevoked))?;

    if claims.uid != user.id.to_string() || claims.ver != user.token_version {
        return Err(AuthError::TokenRevoked.into());
    }

    req.extensions_mut().insert(AuthUser { user });

    Ok(next.run(req).await)
}

pub async fn require_global_admin(
    axum::extract::State(_app_state): axum::extract::State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    if !auth_user.user.is_global_admin {
        return Err(AuthError::RequiresGlobalAdmin.into());
    }
    Ok(next.run(req).await)
}

pub async fn check_privilege(
    app_state: &AppState,
    auth_user: &AuthUser,
    db_name: &str,
    required_role: UserRole,
) -> Result<(), AppError> {
    if auth_user.user.is_global_admin {
        return Ok(());
    }

    let role = app_state.db.read_users(|repo| {
        repo.get_privilege(crate::models::id::UserId(auth_user.user.id), db_name)
    })?;

    if let Some(r) = role
        && r as u8 >= required_role as u8
    {
        return Ok(());
    }

    Err(AuthError::InsufficientPrivilege {
        db_name: db_name.to_string(),
        required: required_role,
    }
    .into())
}
