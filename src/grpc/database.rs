use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::pb::{self, database_service_server::DatabaseService};
use crate::Kasane;

#[tonic::async_trait]
impl DatabaseService for Kasane {
    async fn create(
        &self,
        request: Request<pb::CreateDatabaseRequest>,
    ) -> Result<Response<pb::DatabaseInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let res =
            crate::services::database::create(self, &user, &req.name, req.description).await?;
        Ok(Response::new(res.into()))
    }

    async fn get(
        &self,
        request: Request<pb::GetDatabaseRequest>,
    ) -> Result<Response<pb::DatabaseInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let res = crate::services::database::info(self, &user, &req.db_name).await?;
        Ok(Response::new(res.into()))
    }

    async fn list(
        &self,
        request: Request<pb::ListDatabasesRequest>,
    ) -> Result<Response<pb::ListDatabasesResponse>, Status> {
        let user = authenticate(self, &request).await?;

        let res = crate::services::database::list(self, &user).await?;
        Ok(Response::new(pb::ListDatabasesResponse {
            databases: res.into_iter().map(Into::into).collect(),
        }))
    }

    async fn update(
        &self,
        request: Request<pb::UpdateDatabaseRequest>,
    ) -> Result<Response<pb::DatabaseInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let description = req.description_update.map(Into::into);

        let res =
            crate::services::database::update(self, &user, &req.db_name, req.new_name, description)
                .await?;
        Ok(Response::new(res.into()))
    }

    async fn delete(
        &self,
        request: Request<pb::DeleteDatabaseRequest>,
    ) -> Result<Response<()>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        crate::services::database::remove(self, &user, &req.db_name).await?;
        Ok(Response::new(()))
    }
}
