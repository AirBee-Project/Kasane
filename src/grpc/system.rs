use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::pb::{
    GetSystemInfoRequest, GetSystemInfoResponse, system_service_server::SystemService,
};
use crate::Kasane;

#[tonic::async_trait]
impl SystemService for Kasane {
    async fn get_system_info(
        &self,
        request: Request<GetSystemInfoRequest>,
    ) -> Result<Response<GetSystemInfoResponse>, Status> {
        authenticate(self, &request).await?;

        Ok(Response::new(GetSystemInfoResponse {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }))
    }
}
