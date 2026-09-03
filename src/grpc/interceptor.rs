use tonic::{Request, Status};

use crate::error::{AppError, AuthError};
use crate::models::auth::Claims;
use crate::services::auth::verify_jwt;

/// `tonic::service::Interceptor` は同期処理しかできないため、JWT の署名検証まではここで
/// 行い、ユーザーレコードとの突き合わせ（DB 読み取りが要る）は各 RPC ハンドラが
/// [`super::auth_ctx::load_auth_user`] で行う。
pub fn require_auth(mut request: Request<()>) -> Result<Request<()>, Status> {
    let claims = extract_claims(&request)?;
    request.extensions_mut().insert(claims);
    Ok(request)
}

fn extract_claims(request: &Request<()>) -> Result<Claims, AppError> {
    let value = request
        .metadata()
        .get("authorization")
        .ok_or(AuthError::MissingToken)?;
    let value = value.to_str().map_err(|_| AuthError::MalformedHeader)?;
    let token = value
        .strip_prefix("Bearer ")
        .ok_or(AuthError::MalformedHeader)?;
    verify_jwt(token)
}
