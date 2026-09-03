use tonic::{Request, Response, Status};

use super::pb::{LoginRequest, LoginResponse, auth_service_server::AuthService};
use crate::AppState;
use crate::error::{AppError, AuthError};
use crate::repositories::{CatalogRepository, Storage};
use crate::services::auth::{dummy_verify_password, generate_jwt, verify_password};

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

        let token = generate_jwt(&self.app_state, &payload.username).await?;

        Ok(Response::new(LoginResponse { token }))
    }
}
