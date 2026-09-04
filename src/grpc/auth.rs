//! gRPC 認証サービスおよびインターセプター・認証ヘルパー。
//!
//! - [`AuthServiceImpl`] - ログインエンドポイント。
//! - [`require_auth`] - 保護された RPC の前段で動作する同期インターセプター（署名・有効期限の検証）。
//! - [`authenticate`] - 各 RPC ハンドラー内で最新の利用者レコードを復元するヘルパー。

use tonic::{Request, Response, Status};

use super::pb::{LoginRequest, LoginResponse, auth_service_server::AuthService};
use crate::AppState;
use crate::error::{AppError, AuthError};
use crate::models::auth::Claims;
use crate::repositories::{CatalogRepository, ReadRepository, Storage};
use crate::services::auth::{
    AuthUser, dummy_verify_password, generate_jwt, verify_jwt, verify_password,
};

pub struct AuthServiceImpl {
    pub app_state: AppState,
}

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let payload = request.into_inner();

        let username = payload.username.clone();
        let stored_hash = self
            .app_state
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

        let token = generate_jwt(&self.app_state, &payload.username).await?;

        Ok(Response::new(LoginResponse { token }))
    }
}

/// `tonic::service::Interceptor` は同期処理しかできないため、JWT の署名検証まではここで行い、ユーザーレコードとの突き合わせ（DB 読み取りが要る）は各 RPC ハンドラが [`authenticate`] / [`load_auth_user`] で行う。
pub fn require_auth(mut request: Request<()>) -> Result<Request<()>, Status> {
    let claims = extract_claims(&request)?;
    request.extensions_mut().insert(claims);
    Ok(request)
}

fn extract_claims(request: &Request<()>) -> Result<Claims, AppError> {
    let value = match request.metadata().get("authorization") {
        Some(v) => v,
        None => {
            tracing::warn!("extract_claims: 'authorization' metadata is missing");
            return Err(AuthError::MissingToken.into());
        }
    };
    let value = match value.to_str() {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                "extract_claims: 'authorization' metadata is not valid ASCII/UTF-8: {e}"
            );
            return Err(AuthError::MalformedHeader.into());
        }
    };
    let token = match value.strip_prefix("Bearer ") {
        Some(t) => t,
        None => {
            tracing::warn!(
                "extract_claims: 'authorization' metadata does not start with 'Bearer ': {value:?}"
            );
            return Err(AuthError::MalformedHeader.into());
        }
    };
    verify_jwt(token)
}

/// [`claims_from`] と [`load_auth_user`] を続けて行う。認証必須の RPC の入口で使う。
pub async fn authenticate<T>(
    app_state: &AppState,
    request: &tonic::Request<T>,
) -> Result<AuthUser, Status> {
    let claims = claims_from(request)?;
    load_auth_user(app_state, &claims).await
}

/// [`require_auth`] が検証した `Claims` から、現在の利用者レコードを
/// 読み直して [`AuthUser`] を組み立てる。`uid`/`ver` が最新のレコードと一致しない場合（パスワード変更などでトークンが失効した場合）は拒否する。
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
