use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::pb::{self, system_service_server::SystemService};

#[tonic::async_trait]
impl SystemService for super::Service {
    async fn get_system_info(
        &self,
        request: Request<pb::GetSystemInfoRequest>,
    ) -> Result<Response<pb::GetSystemInfoResponse>, Status> {
        authenticate(&self.0, &request).await?;

        Ok(Response::new(pb::GetSystemInfoResponse {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }))
    }
}
