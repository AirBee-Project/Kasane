use tonic::{Request, Response, Status};

use super::auth_ctx::authenticate;
use super::pb::{GetSystemInfoRequest, GetSystemInfoResponse, system_service_server::SystemService};
use crate::AppState;

pub struct SystemServiceImpl {
    pub app_state: AppState,
}

#[tonic::async_trait]
impl SystemService for SystemServiceImpl {
    async fn get_system_info(
        &self,
        request: Request<GetSystemInfoRequest>,
    ) -> Result<Response<GetSystemInfoResponse>, Status> {
        authenticate(&self.app_state, &request).await?;

        Ok(Response::new(GetSystemInfoResponse {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }))
    }
}
