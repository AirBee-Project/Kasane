use tonic::{Request, Response, Status};

use super::auth_ctx::authenticate;
use super::pb::{SystemInfo, system_service_server::SystemService};
use crate::AppState;

pub struct SystemServiceImpl {
    pub app_state: AppState,
}

#[tonic::async_trait]
impl SystemService for SystemServiceImpl {
    async fn get_info(&self, request: Request<()>) -> Result<Response<SystemInfo>, Status> {
        authenticate(&self.app_state, &request).await?;

        Ok(Response::new(SystemInfo {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }))
    }
}
