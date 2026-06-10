use axum::{Extension, extract::Request, http::header, middleware::Next, response::Response};

use crate::models::users::{User, UserRole};
use crate::{AppState, error::AppError, services::auth::verify_jwt};
use redb::ReadableDatabase;

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
            return Err(AppError::Unauthorized(
                "Missing Authorization header".to_string(),
            ));
        }
    };

    if !auth_header.starts_with("Bearer ") {
        return Err(AppError::Unauthorized(
            "Invalid Authorization header format".to_string(),
        ));
    }

    let token = &auth_header[7..];
    let claims = verify_jwt(token)?;

    // Fast path: check in-memory cache
    let user_opt = {
        let cache = app_state.auth_cache.read().await;
        cache.get(&claims.sub)
    };

    let user = match user_opt {
        Some(u) => u,
        None => {
            // Fallback to repository if not in cache.
            let read_txn = app_state.redb.begin_read()?;
            let repo = crate::repositories::users::KasaneUsersRead::new(read_txn);
            let user = repo
                .get_user(&claims.sub)?
                .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

            // Add to cache
            let mut write_cache = app_state.auth_cache.write().await;
            write_cache.insert(claims.sub.clone(), user.clone());
            user
        }
    };

    // トークンが現在のユーザー状態と一致するか検証する。
    // - uid 不一致: 同名ユーザーが削除・再作成された等で別人のトークン → 拒否
    // - ver 不一致: パスワードや権限変更によりトークンが失効済み → 拒否
    if claims.uid != user.id.to_string() || claims.ver != user.token_version {
        return Err(AppError::Unauthorized("Token has been revoked".to_string()));
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
        return Err(AppError::Forbidden(
            "Requires GlobalAdmin privileges".to_string(),
        ));
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

    let role = {
        let read_txn = app_state.redb.begin_read()?;
        let repo = crate::repositories::users::KasaneUsersRead::new(read_txn);
        repo.get_privilege(auth_user.user.id.into_bytes(), db_name)?
    };

    if let Some(r) = role
        && r as u8 >= required_role as u8
    {
        return Ok(());
    }

    Err(AppError::Forbidden(
        "Insufficient privileges for this database".to_string(),
    ))
}
