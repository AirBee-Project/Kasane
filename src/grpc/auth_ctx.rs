use tonic::Status;

use crate::AppState;
use crate::error::{AppError, AuthError};
use crate::services::auth::AuthUser;
use crate::models::auth::Claims;
use crate::repositories::{ReadRepository, Storage};

/// [`super::interceptor::require_auth`] が検証した `Claims` から、現在の利用者レコードを
/// 読み直して [`AuthUser`] を組み立てる。`uid`/`ver` が最新のレコードと一致しない場合
/// （パスワード変更などでトークンが失効した場合）は拒否する。
pub async fn load_auth_user(app_state: &AppState, claims: &Claims) -> Result<AuthUser, Status> {
    let sub = claims.sub.clone();
    let user = app_state
        .db
        .read(async move |repo| repo.get_user(&sub).await)
        .await?
        .ok_or(AppError::Auth(AuthError::TokenRevoked))?;

    if claims.uid != user.id.to_string() || claims.ver != user.token_version {
        return Err(AppError::Auth(AuthError::TokenRevoked).into());
    }

    Ok(AuthUser { user })
}

pub fn claims_from<T>(request: &tonic::Request<T>) -> Result<Claims, Status> {
    request
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or_else(|| Status::internal("AuthInterceptor did not run"))
}

/// [`claims_from`] と [`load_auth_user`] を続けて行う。認証必須の RPC の入口で使う。
pub async fn authenticate<T>(
    app_state: &AppState,
    request: &tonic::Request<T>,
) -> Result<AuthUser, Status> {
    let claims = claims_from(request)?;
    load_auth_user(app_state, &claims).await
}
